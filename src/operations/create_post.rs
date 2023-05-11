use anyhow::Result;

use crate::{protocols::Client, store};

use super::utils::find_post_dst_identifier;

pub async fn create_post(
    store: &mut store::Store,
    dst_client: &mut dyn Client,
    operation: store::operations::CreatePostOperation,
) -> Result<()> {
    let reply_identifier = operation.status.reply_src_identifier.and_then(|reply| {
        find_post_dst_identifier(
            &store.users,
            &operation.account_pair.src_origin,
            &reply,
            &operation.account_pair.dst_origin,
        )
    });
    let dst_identifier = dst_client
        .post(
            &operation.status.content,
            &operation.status.facets,
            reply_identifier,
            operation.status.media,
            operation.status.external,
            &operation.status.created_at,
        )
        .await?;
    store
        .get_or_create_dst_mut(&operation.account_pair)
        .statuses
        .insert(
            0,
            store::user::DestinationStatus::Post(store::user::IdentifierPair {
                identifier: dst_identifier,
                src_identifier: operation.status.src_identifier,
            }),
        );
    Ok(())
}
