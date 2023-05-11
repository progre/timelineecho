use anyhow::Result;
use tracing::warn;

use crate::{protocols::Client, store};

use super::utils::{find_post_dst_identifier, find_post_dst_identifier_by_uri};

pub async fn create_repost(
    store: &mut store::Store,
    dst_client: &mut dyn Client,
    operation: store::operations::CreateRepostOperation,
) -> Result<()> {
    let target_dst_identifier = find_post_dst_identifier(
        &store.users,
        &operation.account_pair.src_origin,
        &operation.status.target_src_identifier,
        &operation.account_pair.dst_origin,
    )
    .or_else(|| {
        find_post_dst_identifier_by_uri(
            &store.users,
            &operation.status.target_src_uri,
            &operation.account_pair.dst_origin,
        )
    });
    let Some(target_dst_identifier) = target_dst_identifier else {
        warn!("target_dst_identifier not found (target_src_identifier={})", operation.status.target_src_identifier);
        return Ok(());
    };
    let dst_identifier = dst_client
        .repost(target_dst_identifier, &operation.status.created_at)
        .await?;
    store
        .get_or_create_dst_mut(&operation.account_pair)
        .statuses
        .insert(
            0,
            store::user::DestinationStatus::Repost(store::user::DestinationRepost {
                identifier: dst_identifier,
                src_identifier: operation.status.src_identifier,
            }),
        );
    Ok(())
}
