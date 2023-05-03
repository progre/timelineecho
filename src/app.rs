use std::{collections::HashMap, sync::Arc};

use ::config::FileFormat;
use anyhow::{Ok, Result};
use tokio::fs;

use crate::{
    config::Config,
    destination::post,
    protocols::{create_client, create_clients},
    sources::source::{create_store_operations, fetch_statuses, to_dst_statuses},
    store,
};

pub async fn commit(store: &store::Store) -> Result<()> {
    Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
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
    let mut all_operations = Vec::new();
    for config_user in &config.users {
        let mut src_client = create_client(http_client.clone(), &config_user.src).await?;

        let stored_user = store.get_or_create_user(src_client.origin(), src_client.identifier());
        let src = &mut stored_user.src;

        let (statuses, operations) =
            fetch_statuses(http_client.as_ref(), src_client.as_mut(), &src.statuses).await?;
        src.statuses = statuses;

        let mut new_operations = if operations.is_empty() {
            vec![]
        } else {
            let dst_clients = create_clients(&http_client, &config_user.dsts).await?;
            let dsts = to_dst_statuses(dst_clients.as_ref(), &*stored_user, &*src_client);
            let new_operations = create_store_operations(&operations, &dsts);
            for dst_client in dst_clients {
                dst_client_map.insert(
                    store::AccountPair::from_clients(src_client.as_ref(), dst_client.as_ref()),
                    dst_client,
                );
            }
            new_operations
        };

        all_operations.append(&mut new_operations);
    }
    store.operations = all_operations;

    commit(&store).await?;
    post(&mut store, &mut dst_client_map).await?;
    store.retain_all_dst_statuses();
    commit(&store).await?;

    Ok(())
}
