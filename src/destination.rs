use std::collections::HashMap;

use anyhow::Result;

use crate::{
    app::AccountKey,
    database::Database,
    protocols::Client,
    store::{
        self,
        operation::Operation::{Create, Delete, Update},
    },
};

fn to_dst_identifier<'a>(
    src_identifier: &str,
    dst_statuses: &'a [store::user::DestinationStatus],
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
    stored_dst: &mut store::user::Destination,
    dst_client: &mut dyn Client,
    operation: store::operation::Operation,
) -> Result<()> {
    match operation {
        Create(content) => {
            let store::operation::CreatingStatus {
                src_identifier,
                content,
                facets,
                reply_src_identifier,
                media,
                external,
                created_at,
            } = content.status;
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
                store::user::DestinationStatus {
                    identifier: dst_identifier,
                    src_identifier,
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

fn pop_operation(
    store: &mut store::Store,
) -> Option<(store::operation::AccountPair, store::operation::Operation)> {
    let user = store
        .users
        .iter_mut()
        .find(|user| user.dsts.iter().any(|dst| !dst.operations.is_empty()))?;
    let dst = user
        .dsts
        .iter_mut()
        .find(|dst| !dst.operations.is_empty())?;
    let account_pair = store::operation::AccountPair {
        src_origin: user.src.origin.clone(),
        src_account_identifier: user.src.identifier.clone(),
        dst_origin: dst.origin.clone(),
        dst_account_identifier: dst.identifier.clone(),
    };
    Some((account_pair, dst.operations.pop().unwrap()))
}

pub async fn post(
    database: &impl Database,
    store: &mut store::Store,
    dst_clients_map: &mut HashMap<AccountKey, Vec<Box<dyn Client>>>,
) -> Result<()> {
    // WTF: DynamoDB の連続アクセス不能問題が解消するまで連続作業を絞る
    for _ in 0..2 {
        let Some((account_pair, operation)) = pop_operation(store) else {
            break;
        };

        let stored_dst = store.get_or_create_dst(&account_pair);
        let dst_client = dst_clients_map
            .get_mut(&account_pair.to_src_key())
            .unwrap()
            .iter_mut()
            .find(|dst_client| dst_client.to_account_key() == account_pair.to_dst_key())
            .unwrap();

        post_operation(stored_dst, dst_client.as_mut(), operation).await?;
        database.commit(store).await?;
    }

    Ok(())
}
