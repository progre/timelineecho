use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::join_all;
use serde_json::{json, Value};

use crate::{sources::source, store};

use super::twitter_api::{Api, TweetBody};

pub const ORIGIN: &str = "https://twitter.com";

pub struct Client {
    http_client: Arc<reqwest::Client>,
    api: Api,
    user_id: String,
}

impl Client {
    pub async fn new(
        api_key: String,
        api_key_secret: String,
        access_token: String,
        access_token_secret: String,
    ) -> Result<Self> {
        let http_client = Arc::new(reqwest::Client::new());
        let api = Api::new(
            http_client.clone(),
            api_key,
            api_key_secret,
            access_token,
            access_token_secret,
        );
        let json: Value = api.verify_credentials().await?;
        let user_id = json
            .get("id_str")
            .ok_or_else(|| anyhow!("id_str is not found"))?
            .as_str()
            .ok_or_else(|| anyhow!("id_str is not str"))?
            .to_owned();

        Ok(Self {
            http_client,
            api,
            user_id,
        })
    }
}

#[async_trait(?Send)]
impl super::Client for Client {
    fn origin(&self) -> &str {
        ORIGIN
    }

    fn identifier(&self) -> &str {
        &self.user_id
    }

    async fn fetch_statuses(&mut self) -> Result<Vec<source::LiveStatus>> {
        todo!()
    }

    async fn post(
        &mut self,
        content: &str,
        _facets: &[store::Facet],
        reply_identifier: Option<&str>,
        images: Vec<store::Medium>,
        _external: Option<store::External>,
        _created_at: &str,
    ) -> Result<String> {
        let media = if images.is_empty() {
            None
        } else {
            // TODO: alt
            let media_ids = join_all(images.into_iter().map(|image| async {
                let resp = self.http_client.get(image.url).send().await?;
                let res: Value = self.api.upload(resp).await?;
                Ok(res)
            }))
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|res: Value| {
                Ok(res
                    .get("media_id_string")
                    .ok_or_else(|| anyhow!("media_id_string is not found"))?
                    .as_str()
                    .ok_or_else(|| anyhow!("media_id_string is not str"))?
                    .to_owned())
            })
            .collect::<Result<Vec<_>>>()?;
            Some(json!({ "media_ids": media_ids }))
        };

        let body = TweetBody {
            media,
            reply: reply_identifier.map(|reply_identifier| {
                serde_json::json!({ "in_reply_to_tweet_id": reply_identifier })
            }),
            text: content,
        };

        let json: Value = self.api.create_tweet(body).await?;
        let id = json
            .get("data")
            .ok_or_else(|| anyhow!("data is not found"))?
            .as_object()
            .ok_or_else(|| anyhow!("data is not object"))?
            .get("id")
            .ok_or_else(|| anyhow!("id is not found"))?
            .as_str()
            .ok_or_else(|| anyhow!("id is not str"))?;
        Ok(id.to_owned())
    }

    async fn delete(&mut self, identifier: &str) -> Result<()> {
        let _: Value = self.api.destroy_status(identifier).await?;
        Ok(())
    }
}
