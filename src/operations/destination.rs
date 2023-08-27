use std::collections::HashMap;

use anyhow::{bail, Result};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace};

use crate::{
    app::AccountKey,
    protocols::Client,
    store::{
        self,
        operations::{
            AccountPair,
            Operation::{CreatePost, CreateRepost, DeletePost, DeleteRepost, UpdatePost},
        },
    },
};

use super::{
    create_post::create_post, create_repost::create_repost, delete_post::delete_post,
    delete_repost::delete_repost,
};

fn find_dst_client<'a>(
    dst_clients_map: &'a mut HashMap<AccountKey, Vec<Box<dyn Client>>>,
    account_pair: &AccountPair,
) -> Option<&'a mut dyn Client> {
    Some(
        dst_clients_map
            .get_mut(&account_pair.to_src_key())?
            .iter_mut()
            .find(|dst_client| dst_client.to_account_key() == account_pair.to_dst_key())?
            .as_mut(),
    )
}

pub async fn post(
    cancellation_token: &CancellationToken,
    store: &mut store::Store,
    dst_clients_map: &mut HashMap<AccountKey, Vec<Box<dyn Client>>>,
) -> Result<()> {
    trace!("post");
    loop {
        trace!("post loop");
        if cancellation_token.is_cancelled() {
            debug!("cancel accepted");
            return Ok(());
        }
        let Some(operation) = store.operations.pop() else {
            trace!("post completed");
            return Ok(());
        };

        let dst_client = find_dst_client(dst_clients_map, operation.account_pair()).unwrap();

        let result = match operation {
            CreatePost(operation) => create_post(store, dst_client, operation).await,
            CreateRepost(operation) => create_repost(store, dst_client, operation).await,
            UpdatePost(_) => todo!(),
            DeletePost(operation) => delete_post(store, dst_client, operation).await,
            DeleteRepost(operation) => delete_repost(store, dst_client, operation).await,
        };
        if let Err(err) = result {
            error!("{:?}", err);
            bail!("post failed");
        }
    }
}
