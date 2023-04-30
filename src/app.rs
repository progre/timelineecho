use ::config::FileFormat;
use anyhow::{Ok, Result};
use futures::future::join_all;
use tokio::fs;

use crate::{
    config::{self, Config},
    destination::post,
    protocols::create_client,
    source::get,
    store::{self, Store},
};

pub async fn commit(store: &Store) -> Result<()> {
    Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
}

async fn execute_per_user(config_user: &config::User, store: &mut store::Store) -> Result<()> {
    let mut src_client = create_client(&config_user.src).await?;

    let mut dst_clients = join_all(config_user.dsts.iter().map(create_client))
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    get(store, &mut src_client, &mut dst_clients).await?;

    post(store, src_client.as_ref(), &mut dst_clients).await?;

    Ok(())
}

pub async fn app() -> Result<()> {
    let config: Config = ::config::Config::builder()
        .add_source(::config::File::with_name("config.json").format(FileFormat::Json5))
        .build()?
        .try_deserialize()?;

    let mut store: Store =
        serde_json::from_str(&fs::read_to_string("store.json").await.unwrap_or_default())
            .unwrap_or_default();

    for config_user in &config.users {
        execute_per_user(config_user, &mut store).await?;
    }

    Ok(())
}
