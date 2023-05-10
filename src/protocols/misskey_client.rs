use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use linkify::LinkFinder;
use serde_json::{json, Value};

use crate::{sources::source, store};

fn get_value<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| anyhow!("{} is not found", key))
}

fn get_as_string_opt(value: &Value, key: &str) -> Result<Option<String>> {
    Ok(get_value(value, key)?.as_str().map(str::to_owned))
}

fn get_as_string(value: &Value, key: &str) -> Result<String> {
    get_as_string_opt(value, key)?.ok_or_else(|| anyhow!("{} is not str", key))
}

fn get_as_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    get_value(value, key)?
        .as_array()
        .ok_or_else(|| anyhow!("{} is not array", key))
}

fn create_facets(content: &str) -> Vec<store::operations::Facet> {
    LinkFinder::new()
        .links(content)
        .map(|link| store::operations::Facet::Link {
            byte_slice: link.start() as u32..link.end() as u32,
            uri: link.as_str().to_owned(),
        })
        .collect()
}

pub struct Client {
    http_client: Arc<reqwest::Client>,
    origin: String,
    access_token: String,
    user_id: String,
}

impl Client {
    pub async fn new(
        http_client: Arc<reqwest::Client>,
        origin: String,
        access_token: String,
    ) -> Result<Self> {
        let json: Value = http_client
            .post(format!("{}/api/i", origin))
            .json(&json!({ "i": access_token }))
            .send()
            .await?
            .json()
            .await?;
        let user_id = get_as_string(&json, "id")?;
        Ok(Self {
            http_client,
            origin,
            access_token,
            user_id,
        })
    }
}

#[async_trait(?Send)]
impl super::Client for Client {
    fn origin(&self) -> &str {
        &self.origin
    }

    fn identifier(&self) -> &str {
        &self.user_id
    }

    async fn fetch_statuses(&mut self) -> Result<Vec<source::LiveStatus>> {
        let json: Value = self
            .http_client
            .post(format!("{}/api/users/notes", self.origin))
            .json(&json!({ "i": self.access_token, "userId": self.user_id, "limit": 100 }))
            .send()
            .await?
            .json()
            .await?;
        let root = json
            .as_array()
            .ok_or_else(|| anyhow!("root is not array"))?;
        Ok(root
            .iter()
            .map(|item| {
                let content = get_as_string_opt(item, "text")?.unwrap_or_default(); // renote のみの場合は null になる
                let facets = create_facets(&content);
                Ok(source::LiveStatus {
                    identifier: get_as_string(item, "id")?,
                    content,
                    facets,
                    reply_src_identifier: get_as_string_opt(item, "replyId")?,
                    media: get_as_array(item, "files")?
                        .iter()
                        .map(|file| {
                            Ok(store::operations::Medium {
                                url: get_as_string(file, "url")?,
                                alt: get_as_string_opt(file, "comment")?.unwrap_or_default(),
                            })
                        })
                        .collect::<Result<_>>()?,
                    external: source::LiveExternal::Unknown,
                    created_at: DateTime::parse_from_rfc3339(&get_as_string(item, "createdAt")?)?,
                })
            })
            .collect::<Result<_>>()?)
    }

    #[allow(unused)]
    async fn post(
        &mut self,
        content: &str,
        facets: &[store::operations::Facet],
        reply_identifier: Option<&str>,
        images: Vec<store::operations::Medium>,
        external: Option<store::operations::External>,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        todo!();
    }

    #[allow(unused)]
    async fn repost(
        &mut self,
        identifier: &str,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        todo!();
    }

    #[allow(unused)]
    async fn delete(&mut self, identifier: &str) -> Result<()> {
        todo!();
    }
}
