use std::{collections::HashMap, sync::Arc};

use anyhow::{Ok, Result};

use crate::{
    database::Database,
    destination::post,
    sources::source::{get, retain_all_dst_statuses},
};

pub async fn app() -> Result<()> {
    let database = crate::database::DynamoDB::new().await;

    let config = database.config().await?;

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
