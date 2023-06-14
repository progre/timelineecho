use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use megalodon::{megalodon::GetAccountStatusesInputOptions, Megalodon};
use reqwest::header::HeaderMap;
use tracing::{event_enabled, trace, Level};

use crate::{sources::source, store};

fn trace_header(header: &HeaderMap) {
    if !event_enabled!(Level::TRACE) {
        return;
    }
    header
        .iter()
        .filter(|(key, _)| {
            [
                "date",
                "x-ratelimit-limit",
                "x-ratelimit-remaining",
                "x-ratelimit-reset",
            ]
            .contains(&key.as_str())
        })
        .for_each(|(key, value)| {
            trace!("{}: {}", key, value.to_str().unwrap_or_default());
        });
}

pub struct Client {
    origin: String,
    megalodon: Box<dyn Megalodon>,
    account_id: String,
}

impl Client {
    pub async fn new_mastodon(origin: String, access_token: String) -> Result<Self> {
        let megalodon = megalodon::generator(
            megalodon::SNS::Mastodon,
            origin.clone(),
            Some(access_token),
            None,
        );
        let resp = megalodon.verify_account_credentials().await?;
        trace_header(&resp.header);
        let account_id = resp.json().id;

        Ok(Self {
            origin,
            megalodon,
            account_id,
        })
    }
}

#[async_trait(?Send)]
impl super::Client for Client {
    fn origin(&self) -> &str {
        &self.origin
    }

    fn identifier(&self) -> &str {
        &self.account_id
    }

    async fn fetch_statuses(&mut self) -> Result<Vec<source::LiveStatus>> {
        let resp = self
            .megalodon
            .get_account_statuses(
                self.account_id.clone(),
                Some(&GetAccountStatusesInputOptions {
                    limit: Some(40),
                    // exclude_replies: Some(true), // TODO: include self replies
                    ..Default::default()
                }),
            )
            .await?;
        trace_header(&resp.header);
        let statuses: Vec<_> = resp
            .json()
            .into_iter()
            .map(|status| status.into())
            .collect();

        Ok(statuses)
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
        target_identifier: &str,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        todo!();
    }

    #[allow(unused)]
    async fn delete_post(&mut self, identifier: &str) -> Result<()> {
        todo!();
    }

    #[allow(unused)]
    async fn delete_repost(&mut self, identifier: &str) -> Result<()> {
        todo!();
    }
}
