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

fn destination_statuses<'a>(
    users: &'a [store::user::User],
    src_origin: &str,
    dst_origin: &str,
) -> Vec<&'a store::user::DestinationStatus> {
    users
        .iter()
        .filter(|user| user.src.origin == src_origin)
        .flat_map(|user| &user.dsts)
        .filter(|dst| dst.origin == dst_origin)
        .flat_map(|dst| &dst.statuses)
        .collect()
}

fn find_post_dst_identifier<'a>(
    users: &'a [store::user::User],
    src_origin: &str,
    src_identifier: &str,
    dst_origin: &str,
) -> Option<&'a str> {
    Some(
        destination_statuses(users, src_origin, dst_origin)
            .iter()
            .filter_map(|dst_status| match dst_status {
                store::user::DestinationStatus::Post(post) => Some(post),
                store::user::DestinationStatus::Repost(_) => None,
            })
            .find(|dst_post| dst_post.src_identifier == src_identifier)?
            .identifier
            .as_str(),
    )
}

fn find_repost_dst_identifier<'a>(
    users: &'a [store::user::User],
    src_origin: &str,
    src_identifier: &str,
    dst_origin: &str,
) -> Option<&'a str> {
    Some(
        destination_statuses(users, src_origin, dst_origin)
            .iter()
            .filter_map(|dst_status| match dst_status {
                store::user::DestinationStatus::Post(_) => None,
                store::user::DestinationStatus::Repost(repost) => Some(repost),
            })
            .find(|dst_post| dst_post.src_identifier == src_identifier)?
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
            let reply_identifier = reply_src_identifier.and_then(|reply| {
                find_post_dst_identifier(&store.users, &account_pair.src_origin, &reply, &account_pair.dst_origin)
            });
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
                store::user::DestinationStatus::Post(store::user::IdentifierPair {
                    identifier: dst_identifier,
                    src_identifier,
                }),
            );
        }
        CreateRepost(store::operations::CreateRepostOperation {
            account_pair,
            status:
                store::operations::CreateRepostOperationStatus {
                    src_identifier,
                    target_src_identifier,
                    created_at,
                },
        }) => {
            let Some(target_dst_identifier) = find_post_dst_identifier(
                &store.users, 
                &account_pair.src_origin,
                &target_src_identifier,
                &account_pair.dst_origin,
            ) else {
                warn!("target_dst_identifier not found (target_src_identifier={})", target_src_identifier);
                return Ok(());
            };
            let dst_identifier = dst_client
                .repost(target_dst_identifier, &created_at)
                .await?;
            store.get_or_create_dst_mut(&account_pair).statuses.insert(
                0,
                store::user::DestinationStatus::Repost(store::user::IdentifierPair {
                    identifier: dst_identifier,
                    src_identifier,
                }),
            );
        }
        UpdatePost(store::operations::UpdatePostOperation {
            account_pair: _,
            status: _,
        }) => todo!(),
        DeletePost(store::operations::DeletePostOperation {
            account_pair,
            status: store::operations::DeletePostOperationStatus { src_identifier },
        }) => {
            let Some(dst_identifier) = find_post_dst_identifier(
                &store.users,
                &account_pair.src_origin,
                &src_identifier,
                &account_pair.dst_origin,
            ) else {
                warn!("dst_identifier not found (src_identifier={})", src_identifier);
                return Ok(());
            };
            dst_client.delete_post(dst_identifier).await?;
        }
        store::operations::Operation::DeleteRepost(ope) => {
            let Some(dst_identifier) = find_repost_dst_identifier(
                &store.users,
                &ope.account_pair.src_origin,
                &ope.status.src_identifier,
                &ope.account_pair.dst_origin,
            ) else {
                warn!("dst_identifier not found (src_identifier={})", ope.status.src_identifier);
                return Ok(());
            };
            dst_client.delete_repost(dst_identifier).await?;
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
