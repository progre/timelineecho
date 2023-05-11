use std::{collections::HashMap, sync::Arc};

use anyhow::{Ok, Result};

use crate::{
    database::Database,
    operations::destination::post,
    sources::source::{get, retain_all_dst_statuses},
};

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct AccountKey {
    pub origin: String,
    pub identifier: String,
}

pub async fn app(database: &impl Database) -> Result<()> {
    let config = database.config().await?;

    let mut store = database.fetch().await.unwrap_or_default();

    let http_client = Arc::new(reqwest::Client::new());
    let mut dst_client_map = HashMap::new();
    for config_user in &config.users {
        get(
            database,
            &http_client,
            config_user,
            &mut store,
            &mut dst_client_map,
        )
        .await?;
    }
    post(database, &mut store, &mut dst_client_map).await?;
    if store.operations.is_empty() {
        retain_all_dst_statuses(database, &mut store).await?;
    }

    Ok(())
}
