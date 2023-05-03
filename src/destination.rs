use std::collections::HashMap;

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
            account_pair: _,
            source_status,
        }) => {
            let dst_statuses = &mut stored_dst.statuses;
            let reply_identifier = source_status
                .reply_src_identifier
                .and_then(|reply| to_dst_identifier(&reply, dst_statuses.as_ref()));
            let identifier = dst_client
                .post(
                    &source_status.content,
                    &source_status.facets,
                    reply_identifier,
                    source_status.media,
                    source_status.external,
                    &source_status.created_at,
                )
                .await?;
            dst_statuses.insert(
                0,
                store::DestinationStatus {
                    identifier,
                    src_identifier: source_status.src_identifier,
                },
            );
        }
        Update {
            account_pair: _,
            dst_identifier: _,
            content: _,
            facets: _,
        } => todo!(),
        Delete {
            account_pair: _,
            dst_identifier,
        } => {
            dst_client.delete(&dst_identifier).await?;
        }
    }

    Ok(())
}

pub async fn post(
    store: &mut store::Store,
    dst_client_map: &mut HashMap<store::AccountPair, Box<dyn Client>>,
) -> Result<()> {
    loop {
        let Some(operation) = store.operations.pop() else {
            break;
        };

        let account_pair = operation.account_pair();
        let stored_dst = store.get_or_create_dst(account_pair);
        let dst_client = dst_client_map.get_mut(account_pair).unwrap();

        post_operation(stored_dst, dst_client.as_mut(), operation).await?;
        commit(store).await?;
    }

    Ok(())
}
