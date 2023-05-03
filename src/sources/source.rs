use std::convert::Into;

use anyhow::Result;

use crate::{protocols::Client, store};

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
    Create(store::SourceStatusFull),
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
    pub fn to_store(
        &self,
        account_pair: &store::AccountPair,
        dst_statuses: &[store::DestinationStatus],
    ) -> Option<store::Operation> {
        match self {
            Operation::Create(source_status_full) => {
                Some(store::Operation::Create(store::CreatingStatus {
                    account_pair: account_pair.clone(),
                    source_status: source_status_full.clone(),
                }))
            }
            Operation::Update {
                src_identifier,
                content,
                facets,
            } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_identifier)
                .map(|dst| store::Operation::Update {
                    account_pair: account_pair.clone(),
                    dst_identifier: dst.identifier.clone(),
                    content: content.clone(),
                    facets: facets.clone(),
                }),
            Operation::Delete { src_identifier } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_identifier)
                .map(|dst| store::Operation::Delete {
                    account_pair: account_pair.clone(),
                    dst_identifier: dst.identifier.clone(),
                }),
        }
    }
}

pub async fn fetch_statuses(
    http_client: &reqwest::Client,
    src_client: &mut dyn Client,
    src_statuses: &[store::SourceStatus],
) -> Result<(Vec<store::SourceStatus>, Vec<Operation>)> {
    let live_statuses = src_client.fetch_statuses().await?;

    let operations = create_operations(http_client, &live_statuses, src_statuses).await?;
    let statuses: Vec<_> = live_statuses.into_iter().map(Into::into).collect();
    Ok((statuses, operations))
}

pub fn create_store_operations(
    operations: &[Operation],
    dsts: &[(store::AccountPair, Option<&[store::DestinationStatus]>)],
) -> Vec<store::Operation> {
    operations
        .iter()
        .flat_map(|operation| {
            dsts.iter()
                .filter_map(|(account_pair, statuses)| {
                    const EMPTY: [store::DestinationStatus; 0] = [];
                    operation.to_store(account_pair, statuses.unwrap_or(&EMPTY))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn to_dst_statuses<'a>(
    dst_clients: &'a [Box<dyn Client>],
    stored_user: &'a store::User,
    src_client: &dyn Client,
) -> Vec<(store::AccountPair, Option<&'a [store::DestinationStatus]>)> {
    dst_clients
        .iter()
        .map(|dst_client| {
            let account_pair = store::AccountPair::from_clients(src_client, dst_client.as_ref());
            let dst_statuses = stored_user
                .find_dst(
                    &account_pair.dst_origin,
                    &account_pair.dst_account_identifier,
                )
                .map(|dst| &dst.statuses as &[store::DestinationStatus]);
            (account_pair, dst_statuses)
        })
        .collect()
}
