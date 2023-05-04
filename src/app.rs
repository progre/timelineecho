use std::{collections::HashMap, sync::Arc};

use ::config::FileFormat;
use anyhow::{Ok, Result};

use crate::{
    config::Config,
    database::Database,
    destination::post,
    sources::source::{get, retain_all_dst_statuses},
};

pub async fn app() -> Result<()> {
    let config: Config = ::config::Config::builder()
        .add_source(::config::File::with_name("config.json").format(FileFormat::Json5))
        .build()?
        .try_deserialize()?;

    let database = crate::database::File;

    let mut store = database.fetch().await.unwrap_or_default();

    let http_client = Arc::new(reqwest::Client::new());
    let mut dst_client_map = HashMap::new();
    for config_user in &config.users {
        get(
            &database,
            &http_client,
            config_user,
            &mut store,
            &mut dst_client_map,
        )
        .await?;
    }
    post(&database, &mut store, &mut dst_client_map).await?;
    retain_all_dst_statuses(&database, &mut store).await?;

    Ok(())
}
