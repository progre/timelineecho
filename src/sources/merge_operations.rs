use chrono::DateTime;

use super::source::{DeleteOperation, Operation, UpdateOperation};
use crate::{
    app::AccountKey,
    protocols::Client,
    store::{
        self,
        operations::Operation::{Create, Delete, Update},
    },
};

fn to_store_operations(
    dst_clients: &[Box<dyn Client>],
    operations: &[Operation],
    stored_user: &store::user::User,
    src_account_key: &AccountKey,
) -> Vec<store::operations::Operation> {
    let empty = vec![];
    dst_clients
        .iter()
        .flat_map(|dst_client| {
            let dst_account_key = dst_client.to_account_key();

            let dst_statuses = stored_user
                .find_dst(&dst_account_key)
                .map_or_else(|| &empty, |dst| &dst.statuses);
            let account_pair =
                store::operations::AccountPair::from_keys(src_account_key.clone(), dst_account_key);

            operations
                .iter()
                .filter_map(|operation| operation.to_store(account_pair.clone(), dst_statuses))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn sort_operations(operations: &mut [store::operations::Operation]) {
    operations.sort_by_key(|operation| match operation {
        Create(content) => -DateTime::parse_from_rfc3339(&content.status.created_at)
            .unwrap()
            .timestamp_micros(),
        Update(store::operations::UpdateOperation {
            account_pair: _,
            dst_identifier: _,
            content: _,
            facets: _,
        })
        | Delete(store::operations::DeleteOperation {
            account_pair: _,
            dst_identifier: _,
        }) => i64::MAX,
    });
}

fn to_update_operation_src_identifier(src_operation: &Operation) -> Option<&str> {
    match src_operation {
        Operation::Create(_) | Operation::Delete(DeleteOperation { src_identifier: _ }) => None,
        Operation::Update(UpdateOperation {
            src_identifier,
            content: _,
            facets: _,
        }) => Some(src_identifier),
    }
}

fn to_delete_operation_src_identifier(src_operation: &Operation) -> Option<&str> {
    match src_operation {
        Operation::Create(_)
        | Operation::Update(UpdateOperation {
            src_identifier: _,
            content: _,
            facets: _,
        }) => None,
        Operation::Delete(DeleteOperation { src_identifier }) => Some(src_identifier),
    }
}

fn operation_target_state(content: &store::operations::CreateOperation) -> (AccountKey, &str) {
    (
        content.account_pair.to_src_key(),
        &content.status.src_identifier,
    )
}

pub fn merge_operations(
    store: &mut store::Store,
    dst_clients: &[Box<dyn Client>],
    src_account_key: &AccountKey,
    src_operations: &[Operation],
) {
    let mut new_operations = to_store_operations(
        dst_clients,
        src_operations,
        &*store.get_or_create_user_mut(src_account_key),
        src_account_key,
    );

    let operations = &mut store.operations;

    // 投稿の更新
    src_operations
        .iter()
        .filter_map(to_update_operation_src_identifier)
        .for_each(|_| todo!("もし create が未送信なら、create を書き換える必要がある"));
    // 投稿の削除を適用
    src_operations
        .iter()
        .filter_map(to_delete_operation_src_identifier)
        .for_each(|deleting_status_src_identifier| {
            operations.retain(|dst_operation| match dst_operation {
                Create(content) => {
                    operation_target_state(content)
                        != (src_account_key.clone(), deleting_status_src_identifier)
                }
                Update(store::operations::UpdateOperation {
                    account_pair: _,
                    dst_identifier: _,
                    content: _,
                    facets: _,
                })
                | Delete(store::operations::DeleteOperation {
                    account_pair: _,
                    dst_identifier: _,
                }) => true,
            });
        });
    operations.append(&mut new_operations);
    sort_operations(operations);
}
