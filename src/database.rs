use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;

use crate::store;

#[async_trait]
pub trait Database {
    async fn fetch(&self) -> Result<store::Store>;
    async fn commit(&self, store: &store::Store) -> Result<()>;
}

pub struct File;

#[async_trait]
impl Database for File {
    async fn fetch(&self) -> Result<store::Store> {
        let store = serde_json::from_str(&fs::read_to_string("store.json").await?)?;
        Ok(store)
    }

    async fn commit(&self, store: &store::Store) -> Result<()> {
        Ok(fs::write("store.json", serde_json::to_string_pretty(store)?).await?)
    }
}
