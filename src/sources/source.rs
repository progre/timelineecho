use std::convert::Into;

use anyhow::Result;

use crate::{app::commit, protocols::Client, store};

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

fn necessary_src_identifiers(
    stored_statuses: &[store::SourceStatus],
    operations: &[Operation],
) -> Vec<String> {
    let src_identifiers = stored_statuses
        .iter()
        .map(|status| status.identifier.clone());
    let reply_src_identifiers = operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::Create(create) => Some(create.reply_src_identifier.clone()),
            Operation::Update {
                src_identifier: _,
                content: _,
                facets: _,
            }
            | Operation::Delete { src_identifier: _ } => None,
        })
        .flatten();
    src_identifiers.chain(reply_src_identifiers).collect()
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

fn retain_dst_statuses(stored_user: &mut store::User, necessary_src_identifiers: &[String]) {
    for dst in &mut stored_user.dsts {
        dst.statuses
            .retain(|status| necessary_src_identifiers.contains(&status.src_identifier));
    }
}

pub async fn get(
    http_client: &reqwest::Client,
    store: &mut store::Store,
    src_client: &mut dyn Client,
    dst_clients: &[Box<dyn Client>],
) -> Result<()> {
    let stored_user = store.get_or_create_user(src_client.origin(), src_client.identifier());
    if stored_user
        .dsts
        .iter()
        .any(|dst| !dst.operations.is_empty())
    {
        return Ok(());
    }

    let src = &mut stored_user.src;
    let (statuses, operations) = fetch_statuses(http_client, src_client, &src.statuses).await?;
    let necessary_src_identifiers = necessary_src_identifiers(&statuses, &operations);

    src.statuses = statuses;

    for dst_client in dst_clients {
        let dst = stored_user.get_or_create_dst(dst_client.origin(), dst_client.identifier());

        assert!(dst.operations.is_empty());
        dst.operations = create_store_operations(&operations, &dst.statuses);
    }
    retain_dst_statuses(stored_user, &necessary_src_identifiers);
    commit(store).await?;
    Ok(())
}
