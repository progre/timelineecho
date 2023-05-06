use std::{collections::HashMap, convert::Into, sync::Arc};

use anyhow::Result;

use crate::{
    app::AccountKey,
    config,
    database::Database,
    protocols::{create_client, create_clients, Client},
    store,
};

use super::operation_factory::create_operations;

#[derive(Clone)]
pub enum LiveExternal {
    Some(store::operation::External),
    None,
    Unknown,
}

#[derive(Clone)]
pub struct LiveStatus {
    pub identifier: String,
    pub content: String,
    pub facets: Vec<store::operation::Facet>,
    pub reply_src_identifier: Option<String>,
    pub media: Vec<store::operation::Medium>,
    pub external: LiveExternal,
    pub created_at: String,
}

pub enum Operation {
    Create(store::operation::CreatingStatus),
    Update {
        src_identifier: String,
        content: String,
        facets: Vec<store::operation::Facet>,
    },
    Delete {
        src_identifier: String,
    },
}

impl Operation {
    pub fn to_store(
        &self,
        account_pair: store::operation::AccountPair,
        dst_statuses: &[store::user::DestinationStatus],
    ) -> Option<store::operation::Operation> {
        match self {
            Operation::Create(source_status_full) => Some(store::operation::Operation::Create(
                Box::new(store::operation::Create {
                    account_pair,
                    status: source_status_full.clone(),
                }),
            )),
            Operation::Update {
                src_identifier,
                content,
                facets,
            } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_identifier)
                .map(|dst| store::operation::Operation::Update {
                    account_pair,
                    dst_identifier: dst.identifier.clone(),
                    content: content.clone(),
                    facets: facets.clone(),
                }),
            Operation::Delete { src_identifier } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_identifier)
                .map(|dst| store::operation::Operation::Delete {
                    account_pair,
                    dst_identifier: dst.identifier.clone(),
                }),
        }
    }
}

async fn fetch_statuses(
    src_client: &mut dyn Client,
    http_client: &reqwest::Client,
    src_statuses: &[store::user::SourceStatus],
) -> Result<(Vec<store::user::SourceStatus>, Vec<Operation>)> {
    let live_statuses = src_client.fetch_statuses().await?;

    let operations = create_operations(http_client, &live_statuses, src_statuses).await?;
    let statuses: Vec<_> = live_statuses.into_iter().map(Into::into).collect();
    Ok((statuses, operations))
}

fn has_users_operations(operations: &[store::operation::Operation], src_key: &AccountKey) -> bool {
    operations
        .iter()
        .any(|operation| &operation.account_pair().to_src_key() == src_key)
}

fn to_store_operations(
    dst_clients: &[Box<dyn Client>],
    operations: &[Operation],
    stored_user: &store::user::User,
    src_account_key: &AccountKey,
) -> Vec<store::operation::Operation> {
    let dst_account_keys = dst_clients
        .iter()
        .map(|dst_client| dst_client.to_account_key());

    let empty = vec![];
    dst_account_keys
        .flat_map(|dst_account_key| {
            let dst_statuses = stored_user
                .find_dst(&dst_account_key)
                .map_or_else(|| &empty, |dst| &dst.statuses);
            let account_pair =
                store::operation::AccountPair::from_keys(src_account_key.clone(), dst_account_key);
            operations
                .iter()
                .filter_map(|operation| operation.to_store(account_pair.clone(), dst_statuses))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub async fn get(
    database: &impl Database,
    http_client: &Arc<reqwest::Client>,
    config_user: &config::User,
    store: &mut store::Store,
    dst_client_map: &mut HashMap<AccountKey, Vec<Box<dyn Client>>>,
) -> Result<()> {
    let mut src_client = create_client(http_client.clone(), &config_user.src).await?;
    let src_account_key = src_client.to_account_key();

    let has_users_operations = has_users_operations(&store.operations, &src_account_key);
    let stored_user = store.get_or_create_user(&src_account_key);
    let src = &mut stored_user.src;
    let initialize = src.statuses.is_empty();

    let (statuses, operations) =
        fetch_statuses(src_client.as_mut(), http_client.as_ref(), &src.statuses).await?;
    src.statuses = statuses;

    if !operations.is_empty() || has_users_operations {
        let dst_clients = create_clients(http_client, &config_user.dsts).await?;
        if !operations.is_empty() {
            let mut new_operations =
                to_store_operations(&dst_clients, &operations, &*stored_user, &src_account_key);
            store.operations.append(&mut new_operations);
        }
        dst_client_map.insert(src_client.to_account_key(), dst_clients);
    }
    if initialize || !operations.is_empty() {
        database.commit(&*store).await?;
    }
    Ok(())
}

fn necessary_src_identifiers(store: &store::Store) -> Vec<String> {
    store
        .users
        .iter()
        .flat_map(|user| {
            user.src
                .statuses
                .iter()
                .map(|src_status| src_status.identifier.clone())
        })
        .collect()
}

pub async fn retain_all_dst_statuses(
    database: &impl Database,
    store: &mut store::Store,
) -> Result<()> {
    let necessary_src_identifiers = necessary_src_identifiers(&*store);

    let mut updated = false;
    for user in &mut store.users {
        for dst in &mut user.dsts {
            let len = dst.statuses.len();
            dst.statuses
                .retain(|status| necessary_src_identifiers.contains(&status.src_identifier));
            if dst.statuses.len() != len {
                updated = true;
            }
        }
    }
    if updated {
        database.commit(&*store).await?;
    }
    Ok(())
}
