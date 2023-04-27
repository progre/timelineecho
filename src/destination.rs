use anyhow::{anyhow, Result};

use crate::{
    app::commit,
    config::Account,
    protocols::at_proto_client::{self, Client},
    store::{
        self,
        Operation::{Create, Delete, Update},
    },
};

async fn post_per_dst(
    src_origin: &str,
    src_identifier: &str,
    client: &mut Client,
    store: &mut store::Store,
) -> Result<()> {
    loop {
        let stored_dst = store
            .get_or_create_user(src_origin, src_identifier)
            .get_or_create_dst(client.origin(), &client.identifier);
        let Some(operation) = stored_dst.operations.pop() else {
            break;
        };
        match operation {
            Create {
                src_status_identifier,
                content,
                facets,
                reply_src_status_identifier,
                media,
                external,
                created_at,
            } => {
                let dst_statuses = &mut stored_dst.statuses;
                let identifier = client
                    .post(
                        &content,
                        &facets,
                        reply_src_status_identifier.and_then(|reply| {
                            let dst_identifier = &dst_statuses
                                .iter()
                                .find(|dst| dst.src_identifier == reply)?
                                .identifier;
                            Some(dst_identifier.as_str())
                        }),
                        media,
                        external,
                        &created_at,
                    )
                    .await?;
                dst_statuses.insert(
                    0,
                    store::DestinationStatus {
                        identifier,
                        src_identifier: src_status_identifier,
                    },
                );
            }
            Update {
                src_status_identifier: _,
                content: _,
                facets: _,
            } => todo!(),
            Delete { identifier } => {
                client.delete(&identifier).await?;
                let idx = stored_dst
                    .statuses
                    .iter()
                    .position(|status| status.identifier == identifier)
                    .ok_or_else(|| anyhow!("status not found(identifier={})", identifier))?;
                stored_dst.statuses.remove(idx);
            }
        }
        commit(store).await?;
    }
    Ok(())
}

fn client(account: &Account) -> Client {
    match account {
        Account::Mastodon {
            origin: _,
            access_token: _,
        } => {
            todo!();
        }
        Account::AtProtocol {
            origin,
            identifier,
            password,
        } => at_proto_client::Client::new(
            origin.into(),
            reqwest::Client::new(),
            identifier.into(),
            password.into(),
        ),
    }
}

pub async fn post(
    src_origin: &str,
    src_identifier: &str,
    config_dsts: &[Account],
    store: &mut store::Store,
) -> Result<()> {
    let mut clients = config_dsts.iter().map(client).collect::<Vec<_>>();

    for client in &mut clients {
        post_per_dst(src_origin, src_identifier, client, store).await?;
    }

    Ok(())
}
