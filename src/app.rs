use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Ok, Result};
use tokio::{spawn, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace};

use crate::{
    config,
    database::Database,
    operations::destination::post,
    sources::source::{get, retain_all_dst_statuses},
    store,
};

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct AccountKey {
    pub origin: String,
    pub identifier: String,
}

pub async fn do_main_task(
    cancellation_token: &CancellationToken,
    config: &config::Config,
    store: &mut store::Store,
) -> Result<()> {
    trace!("do_main_task");
    let http_client = Arc::new(reqwest::Client::new());
    let mut dst_client_map = HashMap::new();
    for config_user in &config.users {
        get(&http_client, config_user, store, &mut dst_client_map).await?;
    }
    if cancellation_token.is_cancelled() {
        debug!("cancel accepted");
        return Ok(());
    }
    post(cancellation_token, store, &mut dst_client_map).await?;
    if cancellation_token.is_cancelled() {
        debug!("cancel accepted");
        return Ok(());
    }
    if store.operations.is_empty() {
        retain_all_dst_statuses(store).await?;
    }
    trace!("do_main_task completed");

    Ok(())
}

pub async fn app(database: impl Database) -> Result<()> {
    let cancellation_token = CancellationToken::new();
    spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            sleep(Duration::from_secs(20)).await;
            debug!("cancel request");
            cancellation_token.cancel();
        }
    });
    spawn(async move {
        let config = database.config().await?;
        let mut store = database.fetch().await.unwrap_or_default();

        let main_result = do_main_task(&cancellation_token, &config, &mut store).await;

        let commit_result = database.commit(&store).await;
        if let Err(main_error) = main_result {
            if let Err(commit_error) = commit_result {
                error!("commit error: {:?}", commit_error);
            }
            return Err(main_error);
        }

        commit_result
    })
    .await?
}
