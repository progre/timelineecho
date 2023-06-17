use std::{collections::HashMap, time::Duration};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use config::FileFormat;
use serde::{Deserialize, Serialize};
use serde_dynamo::{from_item, to_attribute_value, to_item};
use tokio::{fs, time::sleep};
use tracing::{error, info};

use crate::{config::Config, store};

#[async_trait]
pub trait Database: Send + Sync + 'static {
    async fn config(&self) -> Result<Config>;
    async fn fetch(&self) -> Result<store::Store>;
    async fn commit(&self, store: &store::Store) -> Result<()>;
}

pub struct File;

#[async_trait]
impl Database for File {
    async fn config(&self) -> Result<Config> {
        Ok(::config::Config::builder()
            .add_source(::config::File::with_name("config.json").format(FileFormat::Json5))
            .build()?
            .try_deserialize()?)
    }

    async fn fetch(&self) -> Result<store::Store> {
        let store = serde_json::from_str(&fs::read_to_string("store.json").await?)?;
        Ok(store)
    }

    async fn commit(&self, store: &store::Store) -> Result<()> {
        Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamoDBConfigRoot {
    #[allow(unused)]
    id: u64,
    config_json: String,
}

#[derive(Serialize, Deserialize)]
pub struct DynamoDBRoot {
    id: u64,
    store: store::Store,
}

pub struct DynamoDB {
    client: aws_sdk_dynamodb::Client,
}

impl DynamoDB {
    pub async fn new() -> Self {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_dynamodb::Client::new(&config);
        Self { client }
    }
}

#[async_trait]
impl Database for DynamoDB {
    async fn config(&self) -> Result<Config> {
        let item = self
            .client
            .get_item()
            .table_name("root")
            .set_key(Some(HashMap::from([("id".into(), to_attribute_value(1)?)])))
            .send()
            .await?
            .item()
            .ok_or_else(|| anyhow!("object not found"))?
            .clone();
        let root: DynamoDBConfigRoot = from_item(item)?;
        Ok(serde_json::from_str(&root.config_json)?)
    }

    async fn fetch(&self) -> Result<store::Store> {
        let item = self
            .client
            .get_item()
            .table_name("root")
            .set_key(Some(HashMap::from([("id".into(), to_attribute_value(0)?)])))
            .send()
            .await?
            .item()
            .ok_or_else(|| anyhow!("object not found"))?
            .clone();
        let root: DynamoDBRoot = from_item(item)?;
        Ok(root.store)
    }

    async fn commit(&self, store: &store::Store) -> Result<()> {
        info!("commit to dynamodb...");
        let store = DynamoDBRoot {
            id: 0,
            store: store.clone(),
        };
        let item: HashMap<_, _> = to_item(store)?;
        loop {
            let res = self
                .client
                .put_item()
                .table_name("root")
                .set_item(Some(item.clone()))
                .send()
                .await;
            if let Err(err) = res {
                error!("{:?}", err);
                info!("sleep 10 secs...");
                sleep(Duration::from_secs(10)).await;
                continue;
            }
            break;
        }
        info!("commit succeeded and sleep 10 secs...");
        sleep(Duration::from_secs(10)).await;
        Ok(())
    }
}
