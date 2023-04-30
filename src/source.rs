use std::convert::Into;

use anyhow::Result;

use crate::{app::commit, protocols::Client, store};
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

fn create_operations(
    live_statuses: &[store::CreatingStatus],
    stored_statuses: &[store::SourceStatus],
) -> Vec<Operation> {
    if live_statuses.is_empty() || stored_statuses.is_empty() {
        return vec![];
    }
    // C
    let last_id = stored_statuses
        .iter()
        .max_by_key(|status| &status.identifier)
        .map(|x| &x.identifier);
    let c = live_statuses
        .iter()
        .filter(|live| last_id.map_or(true, |last_id| &live.src_identifier > last_id))
        .map(|status| Operation::Create(status.clone()));

    // UD
    let since_id = &live_statuses
        .iter()
        .min_by_key(|status| &status.src_identifier)
        .unwrap()
        .src_identifier;
    let ud = stored_statuses
        .iter()
        .filter(|stored| &stored.identifier >= since_id)
        .filter_map(|stored| {
            let Some(live) = live_statuses.iter().find(|live| live.src_identifier == stored.identifier) else {
                return Some(Operation::Delete {
                    src_identifier: stored.identifier.clone(),
                });
            };
            if live.content != stored.content {
                return Some(Operation::Update {
                    src_identifier: live.src_identifier.clone(),
                    content: live.content.clone(),
                    facets: live.facets.clone(),
                });
            }
            None
        });

    c.chain(ud).collect()
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
    let operations = create_operations(&statuses, &src.statuses);
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
