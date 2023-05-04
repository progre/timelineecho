use std::{collections::HashMap, sync::Arc};

use ::config::FileFormat;
use anyhow::{Ok, Result};
use tokio::fs;

use crate::{
    config::Config,
    destination::post,
    protocols::{create_client, create_clients, to_account_key},
    sources::source::{self, create_store_operations, fetch_statuses},
    store,
};

pub async fn commit(store: &store::Store) -> Result<()> {
    Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
}

fn has_users_operations(stored_user: &store::User) -> bool {
    stored_user
        .dsts
        .iter()
        .any(|dst| !dst.operations.is_empty())
}

fn update_operations(
    stored_user: &mut store::User,
    dst_account_keys: impl Iterator<Item = store::AccountKey>,
    operations: &[source::Operation],
) {
    for dst_account_key in dst_account_keys {
        let dst = stored_user.get_or_create_dst(&dst_account_key);
        dst.operations = create_store_operations(operations, &dst.statuses);
    }
}

pub async fn app() -> Result<()> {
    let config: Config = ::config::Config::builder()
        .add_source(::config::File::with_name("config.json").format(FileFormat::Json5))
        .build()?
        .try_deserialize()?;

    let mut store: store::Store =
        serde_json::from_str(&fs::read_to_string("store.json").await.unwrap_or_default())
            .unwrap_or_default();

    let http_client = Arc::new(reqwest::Client::new());
    let mut dst_client_map = HashMap::new();
    for config_user in &config.users {
        let mut src_client = create_client(http_client.clone(), &config_user.src).await?;

        let stored_user = store.get_or_create_user(src_client.origin(), src_client.identifier());
        let src = &mut stored_user.src;

        let (statuses, operations) =
            fetch_statuses(http_client.as_ref(), src_client.as_mut(), &src.statuses).await?;
        src.statuses = statuses;

        if !operations.is_empty() || has_users_operations(stored_user) {
            let dst_clients = create_clients(&http_client, &config_user.dsts).await?;
            if !operations.is_empty() {
                let dst_account_keys = dst_clients
                    .iter()
                    .map(|dst_client| to_account_key(dst_client.as_ref()));
                update_operations(stored_user, dst_account_keys, &operations);
            }
            dst_client_map.insert(to_account_key(src_client.as_ref()), dst_clients);
        }

        commit(&store).await?;
    }
    post(&mut store, &mut dst_client_map).await?;
    store.retain_all_dst_statuses();
    commit(&store).await?;

    Ok(())
}
