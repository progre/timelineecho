use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Ok, Result};
use tokio::{spawn, time::sleep};
use tokio_util::sync::CancellationToken;

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

pub async fn do_main_task(
    cancellation_token: &CancellationToken,
    database: &impl Database,
) -> Result<()> {
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
    if cancellation_token.is_cancelled() {
        return Ok(());
    }
    post(
        cancellation_token,
        database,
        &mut store,
        &mut dst_client_map,
    )
    .await?;
    if cancellation_token.is_cancelled() {
        return Ok(());
    }
    if store.operations.is_empty() {
        retain_all_dst_statuses(database, &mut store).await?;
    }

    Ok(())
}

pub async fn app(database: impl Database) -> Result<()> {
    let cancellation_token = CancellationToken::new();
    let cancellation_cancel_token = cancellation_token.clone();
    spawn(async move {
        sleep(Duration::from_secs(30)).await;
        cancellation_cancel_token.cancel();
    });
    spawn(async move { do_main_task(&cancellation_token, &database).await }).await?
}
