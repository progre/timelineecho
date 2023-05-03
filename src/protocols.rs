mod at_proto;
pub mod at_proto_client;
pub mod megalodon_client;
mod misskey_client;
mod twitter_api;
pub mod twitter_client;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::{config, sources::source, store};

#[async_trait(?Send)]
pub trait Client {
    fn origin(&self) -> &str;
    fn identifier(&self) -> &str;

    async fn fetch_statuses(&mut self) -> Result<Vec<source::LiveStatus>>;

    async fn post(
        &mut self,
        content: &str,
        facets: &[store::Facet],
        reply_identifier: Option<&str>,
        images: Vec<store::Medium>,
        external: Option<store::External>,
        created_at: &str,
    ) -> Result<String>;

    async fn delete(&mut self, identifier: &str) -> Result<()>;
}

pub async fn create_client(account: &config::Account) -> Result<Box<dyn Client>> {
    match account {
        config::Account::AtProtocol {
            origin,
            identifier,
            password,
        } => Ok(Box::new(at_proto_client::Client::new(
            origin.into(),
            reqwest::Client::new(),
            identifier.into(),
            password.into(),
        ))),
        config::Account::Mastodon {
            origin,
            access_token,
        } => Ok(Box::new(
            megalodon_client::Client::new_mastodon(origin.clone(), access_token.clone()).await?,
        )),
        config::Account::Misskey {
            origin,
            access_token,
        } => Ok(Box::new(
            misskey_client::Client::new(
                Arc::new(reqwest::Client::new()),
                origin.clone(),
                access_token.clone(),
            )
            .await?,
        )),
        config::Account::Twitter {
            api_key,
            api_key_secret,
            access_token,
            access_token_secret,
        } => Ok(Box::new(
            twitter_client::Client::new(
                api_key.clone(),
                api_key_secret.clone(),
                access_token.clone(),
                access_token_secret.clone(),
            )
            .await?,
        )),
    }
}
