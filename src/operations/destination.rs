use std::collections::HashMap;

use anyhow::Result;

use crate::{
    app::AccountKey,
    database::Database,
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
    database: &impl Database,
    store: &mut store::Store,
    dst_clients_map: &mut HashMap<AccountKey, Vec<Box<dyn Client>>>,
) -> Result<()> {
    // WTF: DynamoDB の連続アクセス不能問題が解消するまで連続作業を絞る
    for _ in 0..2 {
        let Some(operation) = store.operations.pop() else {
            break;
        };

        let dst_client = find_dst_client(dst_clients_map, operation.account_pair()).unwrap();

        match operation {
            CreatePost(operation) => create_post(store, dst_client, operation).await?,
            CreateRepost(operation) => create_repost(store, dst_client, operation).await?,
            UpdatePost(_) => todo!(),
            DeletePost(operation) => delete_post(store, dst_client, operation).await?,
            DeleteRepost(operation) => delete_repost(store, dst_client, operation).await?,
        }
        database.commit(store).await?;
    }
    Ok(())
}
