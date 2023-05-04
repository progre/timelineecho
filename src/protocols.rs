mod at_proto;
pub mod at_proto_client;
pub mod megalodon_client;
mod misskey_client;
mod twitter_api;
pub mod twitter_client;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures::future::join_all;

use crate::{
    config,
    sources::source,
    store::{self, AccountKey},
};

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

pub fn to_account_key(client: &dyn Client) -> AccountKey {
    AccountKey {
        origin: client.origin().to_owned(),
        identifier: client.identifier().to_owned(),
    }
}

pub async fn create_client(
    http_client: Arc<reqwest::Client>,
    account: &config::Account,
) -> Result<Box<dyn Client>> {
    match account {
        config::Account::AtProtocol {
            origin,
            identifier,
            password,
        } => Ok(Box::new(at_proto_client::Client::new(
            origin.into(),
            http_client,
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
            misskey_client::Client::new(http_client, origin.clone(), access_token.clone()).await?,
        )),
        config::Account::Twitter {
            api_key,
            api_key_secret,
            access_token,
            access_token_secret,
        } => Ok(Box::new(
            twitter_client::Client::new(
                http_client,
                api_key.clone(),
                api_key_secret.clone(),
                access_token.clone(),
                access_token_secret.clone(),
            )
            .await?,
        )),
    }
}

pub async fn create_clients(
    http_client: &Arc<reqwest::Client>,
    accounts: &[config::Account],
) -> Result<Vec<Box<dyn Client>>> {
    let clients = accounts
        .iter()
        .map(|dst| create_client(http_client.clone(), dst));
    join_all(clients)
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()
}
