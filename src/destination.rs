use std::collections::HashMap;

use anyhow::Result;
use tracing::warn;

use crate::{
    app::AccountKey,
    database::Database,
    protocols::Client,
    store::{
        self,
        operations::Operation::{CreatePost, CreateRepost, DeletePost, UpdatePost},
    },
};

fn to_dst_identifier<'a>(
    src_origin: &str,
    src_identifier: &str,
    store: &'a store::Store,
) -> Option<&'a str> {
    Some(
        store
            .users
            .iter()
            .filter(|user| user.src.origin == src_origin)
            .flat_map(|user| &user.dsts)
            .flat_map(|dst| &dst.statuses)
            .find(|dst| dst.src_identifier == src_identifier)?
            .identifier
            .as_str(),
    )
}

pub async fn post_operation(
    store: &mut store::Store,
    dst_client: &mut dyn Client,
    operation: store::operations::Operation,
) -> Result<()> {
    match operation {
        CreatePost(store::operations::CreatePostOperation {
            account_pair,
            status:
                store::operations::CreatePostOperationStatus {
                    src_identifier,
                    content,
                    facets,
                    reply_src_identifier,
                    media,
                    external,
                    created_at,
                },
        }) => {
            let reply_identifier = reply_src_identifier
                .and_then(|reply| to_dst_identifier(&account_pair.src_origin, &reply, &*store));
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
            store.get_or_create_dst_mut(&account_pair).statuses.insert(
                0,
                store::user::DestinationStatus {
                    identifier: dst_identifier,
                    src_identifier,
                },
            );
        }
        CreateRepost(store::operations::CreateRepostOperation {
            account_pair,
            status:
                store::operations::CreateRepostOperationStatus {
                    target_src_identifier,
                    created_at,
                },
        }) => {
            let Some(target_dst_identifier) = to_dst_identifier(
                &account_pair.src_origin,
                &target_src_identifier,
                &*store,
            ) else {
                warn!("target_dst_identifier not found (target_src_identifier={})", target_src_identifier);
                return Ok(());
            };
            let _dst_identifier = dst_client
                .repost(target_dst_identifier, &created_at)
                .await?;
        }
        UpdatePost(store::operations::UpdatePostOperation {
            account_pair: _,
            status: _,
        }) => todo!(),
        DeletePost(store::operations::DeletePostOperation {
            account_pair,
            status: store::operations::DeletePostOperationStatus { src_identifier },
        }) => {
            let Some(dst_identifier) = to_dst_identifier(&account_pair.src_origin, &src_identifier, &*store) else {
                warn!("dst_identifier not found (src_identifier={})", src_identifier);
                return Ok(());
            };
            dst_client.delete(dst_identifier).await?;
        }
    }

    Ok(())
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

        let dst_client = dst_clients_map
            .get_mut(&operation.account_pair().to_src_key())
            .unwrap()
            .iter_mut()
            .find(|dst_client| dst_client.to_account_key() == operation.account_pair().to_dst_key())
            .unwrap();

        post_operation(store, dst_client.as_mut(), operation).await?;
        database.commit(store).await?;
    }

    Ok(())
}
