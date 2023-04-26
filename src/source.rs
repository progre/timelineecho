use anyhow::Result;

use crate::{
    config::Account,
    protocols::megalodon_client::{self, Client},
    store,
};

#[derive(Clone)]
pub struct Status {
    pub identifier: String,
    pub content: String,
    pub facets: Vec<store::Facet>,
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

fn create_operations(live_statuses: &[Status], user: Option<&store::User>) -> Vec<Operation> {
    let Some(user) = user else {
        return vec![];
    };
    let stored_statuses = &user.src.statuses;
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
        .filter(|live| {
            if let Some(last_id) = last_id {
                &live.identifier > last_id
            } else {
                true
            }
        })
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

pub async fn fetch_new_statuses(
    account: &Account,
    store: &store::Store,
) -> Result<(String, Vec<store::SourceStatus>, Vec<Operation>)> {
    let mut client = client(account);

    let (identifier, statuses) = client.fetch_statuses().await?;

    let user = store.get_user(account.origin(), &identifier);

    let operations = create_operations(&statuses, user);
    Ok((
        identifier,
        statuses
            .into_iter()
            .map(|status| store::SourceStatus {
                identifier: status.identifier,
                content: status.content,
            })
            .collect(),
        operations,
    ))
}
