use std::convert::Into;

use anyhow::Result;
use futures::future::join_all;

use crate::{app::commit, protocols::Client, store};

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

enum Operation {
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

async fn into_creating_status(live: LiveStatus) -> Result<store::CreatingStatus> {
    let external = match live.external {
        LiveExternal::Some(external) => Some(external),
        LiveExternal::None => None,
        LiveExternal::Unknown => {
            todo!()
        }
    };
    Ok(store::CreatingStatus {
        src_identifier: live.identifier,
        content: live.content,
        facets: live.facets,
        reply_src_identifier: live.reply_src_identifier,
        media: live.media,
        external,
        created_at: live.created_at,
    })
}

async fn create_operations(
    live_statuses: &[LiveStatus],
    stored_statuses: &[store::SourceStatus],
) -> Result<Vec<Operation>> {
    if live_statuses.is_empty() || stored_statuses.is_empty() {
        return Ok(vec![]);
    }
    // C
    let last_id = stored_statuses
        .iter()
        .max_by_key(|status| &status.identifier)
        .map(|x| &x.identifier);
    let c = live_statuses
        .iter()
        .filter(|live| last_id.map_or(true, |last_id| &live.identifier > last_id))
        .map(|status| async {
            Ok(Operation::Create(
                into_creating_status(status.clone()).await?,
            ))
        });
    let c = join_all(c).await.into_iter().collect::<Result<Vec<_>>>()?;
    // UD
    let since_id = &live_statuses
        .iter()
        .min_by_key(|status| &status.identifier)
        .unwrap()
        .identifier;
    let ud = stored_statuses
        .iter()
        .filter(|stored| &stored.identifier >= since_id)
        .filter_map(|stored| {
            let Some(live) = live_statuses.iter().find(|live| live.identifier == stored.identifier) else {
                return Some(Operation::Delete {
                    src_identifier: stored.identifier.clone(),
                });
            };
            if live.content != stored.content {
                return Some(Operation::Update {
                    src_identifier: live.identifier.clone(),
                    content: live.content.clone(),
                    facets: live.facets.clone(),
                });
            }
            None
        });

    Ok(c.into_iter().chain(ud).collect())
}

pub async fn get(
    store: &mut store::Store,
    src_client: &mut Box<dyn Client>,
    dst_clients: &mut [Box<dyn Client>],
) -> Result<()> {
    let statuses = src_client.fetch_statuses().await?;

    let stored_user = store.get_or_create_user(src_client.origin(), src_client.identifier());
    if stored_user
        .dsts
        .iter()
        .any(|dst| !dst.operations.is_empty())
    {
        return Ok(());
    }

    let src = &mut stored_user.src;
    let operations = create_operations(&statuses, &src.statuses).await?;
    src.statuses = statuses.into_iter().map(Into::into).collect();

    let src_identifiers = src.statuses.iter().map(|status| status.identifier.clone());
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
    let necessary_src_identifiers: Vec<_> = src_identifiers.chain(reply_src_identifiers).collect();

    for dst_client in dst_clients {
        let dst = stored_user.get_or_create_dst(dst_client.origin(), dst_client.identifier());

        assert!(dst.operations.is_empty());
        dst.operations = operations
            .iter()
            .filter_map(|operation| operation.to_store(&dst.statuses))
            .collect();

        dst.statuses
            .retain(|status| necessary_src_identifiers.contains(&status.src_identifier));
    }
    commit(store).await?;
    Ok(())
}
