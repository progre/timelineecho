use anyhow::Result;
use itertools::Itertools;

use crate::{
    app::commit,
    config::{self, Account},
    protocols::megalodon_client::{self, Client},
    store,
};

#[derive(Clone)]
pub struct Status {
    pub identifier: String,
    pub content: String,
    pub facets: Vec<store::Facet>,
    pub reply_identifier: Option<String>,
    pub media: Vec<store::Medium>,
    pub external: Option<store::External>,
}

pub enum Operation {
    Create(Status),
    Update {
        src_status_identifier: String,
        content: String,
        facets: Vec<store::Facet>,
    },
    Delete {
        src_status_identifier: String,
    },
}
impl Operation {
    pub fn to_store(&self, dst_statuses: &[store::DestinationStatus]) -> Option<store::Operation> {
        match self {
            Operation::Create(src) => Some(store::Operation::Create {
                src_status_identifier: src.identifier.clone(),
                content: src.content.clone(),
                facets: src.facets.clone(),
                reply_src_status_identifier: src.reply_identifier.clone(),
                media: src.media.clone(),
                external: src.external.clone(),
            }),
            Operation::Update {
                src_status_identifier: _,
                content: _,
                facets: _,
            } => todo!(),
            Operation::Delete {
                src_status_identifier,
            } => dst_statuses
                .iter()
                .find(|dst| &dst.src_identifier == src_status_identifier)
                .map(|dst| store::Operation::Delete {
                    identifier: dst.identifier.clone(),
                }),
        }
    }
}

fn client(account: &Account) -> Client {
    match account {
        Account::Mastodon {
            origin,
            access_token,
        } => megalodon_client::Client::new_mastodon(origin.clone(), access_token.clone()),
        Account::AtProtocol {
            origin: _,
            identifier: _,
            password: _,
        } => {
            todo!()
        }
    }
}

fn create_operations(
    live_statuses: &[Status],
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
        .filter(|live| last_id.map_or(true, |last_id| &live.identifier > last_id))
        .map(|status| Operation::Create(status.clone()));

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
                    src_status_identifier: stored.identifier.clone(),
                });
            };
            if live.content != stored.content {
                return Some(Operation::Update {
                    src_status_identifier: live.identifier.clone(),
                    content: live.content.clone(),
                    facets: live.facets.clone(),
                });
            }
            None
        });

    c.chain(ud).collect()
}

pub async fn get(config_user: &config::User, store: &mut store::Store) -> Result<String> {
    let mut client = client(&config_user.src);
    let (identifier, statuses) = client.fetch_statuses().await?;

    let stored_user = store.get_or_create_user(config_user.src.origin(), &identifier);
    if stored_user
        .dsts
        .iter()
        .any(|dst| !dst.operations.is_empty())
    {
        return Ok(identifier);
    }

    let src = &mut stored_user.src;
    let operations = create_operations(&statuses, &src.statuses);
    let new_statuses: Vec<_> = statuses
        .into_iter()
        .map(|status| store::SourceStatus {
            identifier: status.identifier,
            content: status.content,
        })
        .collect();
    let scoped_src_status_identifiers: Vec<_> = new_statuses
        .iter()
        .chain(&src.statuses)
        .map(|status| status.identifier.clone())
        .unique()
        .collect();
    src.statuses = new_statuses;

    for config_dst in &config_user.dsts {
        let dst = stored_user.get_or_create_dst(config_dst.origin(), config_dst.identifier());

        assert!(dst.operations.is_empty());
        dst.operations = operations
            .iter()
            .filter_map(|operation| operation.to_store(&dst.statuses))
            .collect();

        dst.statuses
            .retain(|status| scoped_src_status_identifiers.contains(&status.src_identifier));
    }
    commit(store).await?;
    Ok(identifier)
}
