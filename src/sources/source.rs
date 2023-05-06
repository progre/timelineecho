use std::{collections::HashMap, convert::Into, sync::Arc};

use anyhow::Result;

use crate::{
    config,
    database::Database,
    protocols::{create_client, create_clients, to_account_key, Client},
    store::{self, AccountKey},
};

use super::operation_factory::create_operations;

#[derive(Clone)]
pub enum LiveExternal {
    Some(store::External),
    None,
    Unknown,
}

#[derive(Clone)]
pub struct LiveStatus {
    pub identifier: String,
    pub content: String,
    pub facets: Vec<store::Facet>,
    pub reply_src_identifier: Option<String>,
    pub media: Vec<store::Medium>,
    pub external: LiveExternal,
    pub created_at: String,
}

pub enum Operation {
    Create(store::CreatingStatus),
    Update {
        src_identifier: String,
        content: String,
        facets: Vec<store::Facet>,
    },
    Delete {
        src_identifier: String,
    },
}

impl Operation {
    pub fn to_store(&self, dst_statuses: &[store::DestinationStatus]) -> Option<store::Operation> {
        match self {
            Operation::Create(source_status_full) => {
                Some(store::Operation::Create(source_status_full.clone()))
            }
            Operation::Update {
                src_identifier,
                content,
                facets,
            } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_identifier)
                .map(|dst| store::Operation::Update {
                    dst_identifier: dst.identifier.clone(),
                    content: content.clone(),
                    facets: facets.clone(),
                }),
            Operation::Delete { src_identifier } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_identifier)
                .map(|dst| store::Operation::Delete {
                    dst_identifier: dst.identifier.clone(),
                }),
        }
    }
}

async fn fetch_statuses(
    http_client: &reqwest::Client,
    src_client: &mut dyn Client,
    src_statuses: &[store::SourceStatus],
) -> Result<(Vec<store::SourceStatus>, Vec<Operation>)> {
    let live_statuses = src_client.fetch_statuses().await?;

    let operations = create_operations(http_client, &live_statuses, src_statuses).await?;
    let statuses: Vec<_> = live_statuses.into_iter().map(Into::into).collect();
    Ok((statuses, operations))
}

fn create_store_operations(
    operations: &[Operation],
    dst_statuses: &[store::DestinationStatus],
) -> Vec<store::Operation> {
    operations
        .iter()
        .filter_map(|operation| operation.to_store(dst_statuses))
        .collect()
}

fn has_users_operations(stored_user: &store::User) -> bool {
    stored_user
        .dsts
        .iter()
        .any(|dst| !dst.operations.is_empty())
}

fn update_operations(
    stored_user: &mut store::User,
    dst_account_keys: impl Iterator<Item = store::AccountKey>,
    operations: &[Operation],
) {
    for dst_account_key in dst_account_keys {
        let dst = stored_user.get_or_create_dst(&dst_account_key);
        dst.operations = create_store_operations(operations, &dst.statuses);
    }
}

pub async fn get(
    database: &impl Database,
    http_client: &Arc<reqwest::Client>,
    config_user: &config::User,
    store: &mut store::Store,
    dst_client_map: &mut HashMap<AccountKey, Vec<Box<dyn Client>>>,
) -> Result<()> {
    let mut src_client = create_client(http_client.clone(), &config_user.src).await?;

    let stored_user = store.get_or_create_user(src_client.origin(), src_client.identifier());
    let src = &mut stored_user.src;

    let (statuses, operations) =
        fetch_statuses(http_client.as_ref(), src_client.as_mut(), &src.statuses).await?;
    src.statuses = statuses;

    if !operations.is_empty() || has_users_operations(&*stored_user) {
        let dst_clients = create_clients(http_client, &config_user.dsts).await?;
        if !operations.is_empty() {
            let dst_account_keys = dst_clients
                .iter()
                .map(|dst_client| to_account_key(dst_client.as_ref()));
            update_operations(stored_user, dst_account_keys, &operations);
            database.commit(&*store).await?;
        }
        dst_client_map.insert(to_account_key(src_client.as_ref()), dst_clients);
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
