use anyhow::Result;
use tracing::warn;

use crate::{protocols::Client, store};

use super::utils::find_repost_dst_identifier;

pub async fn delete_repost(
    store: &mut store::Store,
    dst_client: &mut dyn Client,
    operation: store::operations::DeleteRepostOperation,
) -> Result<()> {
    let dst_identifier = find_repost_dst_identifier(
        &store.users,
        &operation.account_pair.src_origin,
        &operation.status.src_identifier,
        &operation.account_pair.dst_origin,
    );
    let Some(dst_identifier) = dst_identifier else {
        warn!("dst_identifier not found (src_identifier={})", operation.status.src_identifier);
        return Ok(());
    };
    dst_client.delete_repost(dst_identifier).await?;
    Ok(())
}
