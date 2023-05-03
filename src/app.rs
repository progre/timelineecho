use std::{collections::HashMap, sync::Arc};

use ::config::FileFormat;
use anyhow::{Ok, Result};
use futures::future::join_all;
use tokio::fs;

use crate::{
    config::Config, destination::post, protocols::create_client, sources::source::get, store,
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
        let dst_clients = config_user
            .dsts
            .iter()
            .map(|dst| create_client(http_client.clone(), dst));
        let dst_clients = join_all(dst_clients)
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

        get(
            http_client.as_ref(),
            &mut store,
            src_client.as_mut(),
            &dst_clients,
        )
        .await?;

        for dst_client in dst_clients {
            dst_client_map.insert(
                store::AccountPair::from_clients(src_client.as_ref(), dst_client.as_ref()),
                dst_client,
            );
        }
    }
    post(&mut store, &mut dst_client_map).await?;
    store.retain_all_dst_statuses();
    commit(&store).await?;

    Ok(())
}
