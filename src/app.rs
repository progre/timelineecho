use ::config::FileFormat;
use anyhow::{Ok, Result};
use tokio::fs;

use crate::{
    config::{self, Config},
    destination::post,
    source::get,
    store::{self, Store},
};

pub async fn commit(store: &Store) -> Result<()> {
    Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
}

async fn execute_per_user(config_user: &config::User, store: &mut store::Store) -> Result<()> {
    let identifier = get(config_user, store).await?;

    post(
        config_user.src.origin(),
        &identifier,
        &config_user.dsts,
        store,
    )
    .await?;

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
