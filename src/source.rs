use anyhow::Result;

use crate::{
    config::Account,
    protocols::megalodon_client::{self, Client},
    store,
};

pub struct Status {
    pub identifier: String,
    pub content: String,
    pub facets: Vec<store::Facet>,
    pub media: Vec<store::Medium>,
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
) -> Vec<store::Operation> {
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
        .map(|live| store::Operation::Create {
            src_status_idenfitier: live.identifier.clone(),
            content: live.content.clone(),
            facets: live.facets.clone(),
            media: live.media.clone(),
        });

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
                return Some(store::Operation::Delete {
                    src_status_idenfitier: stored.identifier.clone(),
                });
            };
            if live.content != stored.content {
                return Some(store::Operation::Update {
                    src_status_idenfitier: live.identifier.clone(),
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
    stored_users: &[store::User],
) -> Result<(String, Vec<store::SourceStatus>, Vec<store::Operation>)> {
    let mut client = client(account);

    let (identifier, statuses) = client.fetch_statuses().await?;

    let operations = create_operations(
        &statuses,
        stored_users
            .iter()
            .find(|user| user.src.origin == account.origin() && user.src.identifier == identifier)
            .map_or(&[], |user| &user.src.statuses),
    );
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
