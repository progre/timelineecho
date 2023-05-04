use std::{collections::HashMap, sync::Arc};

use ::config::FileFormat;
use anyhow::{Ok, Result};
use tokio::fs;

use crate::{
    config::Config,
    destination::post,
    sources::source::{get, retain_all_dst_statuses},
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
        get(&http_client, config_user, &mut store, &mut dst_client_map).await?;
    }
    post(&mut store, &mut dst_client_map).await?;
    retain_all_dst_statuses(&mut store).await?;

    Ok(())
}
