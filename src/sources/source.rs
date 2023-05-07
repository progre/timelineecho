use std::{collections::HashMap, convert::Into, sync::Arc};

use anyhow::Result;

use crate::{
    app::AccountKey,
    config,
    database::Database,
    protocols::{create_client, create_clients, Client},
    store,
};

use super::{merge_operations::merge_operations, operation_factory::create_operations};

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
    let stored_user = store.get_or_create_user_mut(&src_account_key);
    let src = &mut stored_user.src;
    let initialize = src.statuses.is_empty();

    let (statuses, operations) =
        fetch_statuses(src_client.as_mut(), http_client.as_ref(), &src.statuses).await?;
    src.statuses = statuses;

    if !operations.is_empty() || has_users_operations {
        let dst_clients = create_clients(http_client, &config_user.dsts).await?;
        if !operations.is_empty() {
            merge_operations(store, &dst_clients, &src_account_key, &operations);
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
        .flat_map(|user| user.src.statuses.iter())
        .map(|src_status| src_status.identifier.clone())
        .collect()
}

pub async fn retain_all_dst_statuses(
    database: &impl Database,
    store: &mut store::Store,
) -> Result<()> {
    let necessary_src_identifiers = necessary_src_identifiers(&*store);

    let mut updated = false;
    store
        .users
        .iter_mut()
        .flat_map(|user| user.dsts.iter_mut())
        .for_each(|dst| {
            let len = dst.statuses.len();
            dst.statuses
                .retain(|status| necessary_src_identifiers.contains(&status.src_identifier));
            updated |= dst.statuses.len() != len;
        });
    if updated {
        database.commit(&*store).await?;
    }
    Ok(())
}
