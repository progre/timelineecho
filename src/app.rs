use std::{collections::HashMap, sync::Arc};

use ::config::FileFormat;
use anyhow::{Ok, Result};
use tokio::fs;

use crate::{
    config::Config,
    destination::post,
    protocols::{create_client, create_clients},
    sources::source::{create_store_operations, fetch_statuses},
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
    for config_user in &config.users {
        let mut src_client = create_client(http_client.clone(), &config_user.src).await?;

        let stored_user = store.get_or_create_user(src_client.origin(), src_client.identifier());
        let src = &mut stored_user.src;

        let (statuses, operations) =
            fetch_statuses(http_client.as_ref(), src_client.as_mut(), &src.statuses).await?;
        src.statuses = statuses;

        if !operations.is_empty() {
            create_clients(&http_client, &config_user.dsts)
                .await?
                .into_iter()
                .for_each(|dst_client| {
                    let dst =
                        stored_user.get_or_create_dst(dst_client.origin(), dst_client.identifier());
                    dst.operations = create_store_operations(&operations, &dst.statuses);

                    dst_client_map.insert(
                        store::AccountPair::from_clients(src_client.as_ref(), dst_client.as_ref()),
                        dst_client,
                    );
                });
        }

        commit(&store).await?;
    }
    post(&mut store, &mut dst_client_map).await?;
    store.retain_all_dst_statuses();
    commit(&store).await?;

    Ok(())
}
