use anyhow::Result;

use crate::{
    app::commit,
    protocols::Client,
    store::{
        self,
        Operation::{Create, Delete, Update},
    },
};

async fn post_per_dst(
    store: &mut store::Store,
    src_client: &dyn Client,
    dst_client: &mut Box<dyn Client>,
) -> Result<()> {
    loop {
        let stored_dst = store
            .get_or_create_user(src_client.origin(), src_client.identifier())
            .get_or_create_dst(dst_client.origin(), dst_client.identifier());
        let Some(operation) = stored_dst.operations.pop() else {
            break;
        };
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
                let dst_identifier = dst_client
                    .post(
                        &content,
                        &facets,
                        reply_src_identifier.and_then(|reply| {
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
        commit(store).await?;
    }
    Ok(())
}

pub async fn post(
    store: &mut store::Store,
    src_client: &dyn Client,
    dst_clients: &mut [Box<dyn Client>],
) -> Result<()> {
    for dst_client in dst_clients {
        post_per_dst(store, src_client, dst_client).await?;
    }

    Ok(())
}
