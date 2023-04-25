use ::config::FileFormat;
use anyhow::{Ok, Result};
use tokio::fs;

use crate::{
    config::{self, Config},
    destination::post,
    source::fetch_new_statuses,
    store::{self, Store},
};

pub async fn commit(store: &Store) -> Result<()> {
    Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
}

async fn execute_per_user(config_user: &config::User, store: &mut store::Store) -> Result<()> {
    let (identifier, source_statuses, operations) =
        fetch_new_statuses(&config_user.src, &store.users).await?;
    let stored_user = store.get_or_create_user(config_user.src.origin(), &identifier);

    if stored_user.dsts.iter().all(|dst| dst.operations.is_empty()) {
        stored_user.src.statuses = source_statuses;
        for config_dst in &config_user.dsts {
            let dst = stored_user.get_or_create_dst(config_dst.origin(), config_dst.identifier());
            assert!(dst.operations.is_empty());
            dst.operations = operations.clone();
        }
        commit(store).await?;
    }

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
