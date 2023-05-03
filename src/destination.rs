use anyhow::Result;

use crate::{
    app::commit,
    protocols::Client,
    store::{
        self,
        Operation::{Create, Delete, Update},
    },
};

fn to_dst_identifier<'a>(
    src_identifier: &str,
    dst_statuses: &'a [store::DestinationStatus],
) -> Option<&'a str> {
    Some(
        dst_statuses
            .iter()
            .find(|dst| dst.src_identifier == src_identifier)?
            .identifier
            .as_str(),
    )
}

pub async fn post_operation(
    stored_dst: &mut store::Destination,
    dst_client: &mut dyn Client,
    operation: store::Operation,
) -> Result<()> {
    match operation {
        Create(store::CreatingStatus {
            src_identifier,
            content,
            facets,
            reply_src_identifier,
            media,
            external,
            created_at,
        }) => {
            let dst_statuses = &mut stored_dst.statuses;
            let reply_identifier = reply_src_identifier
                .and_then(|reply| to_dst_identifier(&reply, dst_statuses.as_ref()));
            let dst_identifier = dst_client
                .post(
                    &content,
                    &facets,
                    reply_identifier,
                    media,
                    external,
                    &created_at,
                )
                .await?;
            dst_statuses.insert(
                0,
                store::DestinationStatus {
                    identifier: dst_identifier,
                    src_identifier,
                },
            );
        }
        Update {
            dst_identifier: _,
            content: _,
            facets: _,
        } => todo!(),
        Delete { dst_identifier } => {
            dst_client.delete(&dst_identifier).await?;
        }
    }

    Ok(())
}

pub async fn post(
    store: &mut store::Store,
    src_client: &dyn Client,
    dst_clients: &mut [Box<dyn Client>],
) -> Result<()> {
    for dst_client in dst_clients {
        loop {
            let stored_dst = store
                .get_or_create_user(src_client.origin(), src_client.identifier())
                .get_or_create_dst(dst_client.origin(), dst_client.identifier());
            let Some(operation) = stored_dst.operations.pop() else {
                break;
            };
            post_operation(stored_dst, dst_client.as_mut(), operation).await?;
            commit(store).await?;
        }
    }

    Ok(())
}
