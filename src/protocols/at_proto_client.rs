use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use atrium_api::{
    app,
    com::{
        self,
        atproto::repo::{create_record::CreateRecord, delete_record::DeleteRecord},
    },
};
use chrono::{DateTime, FixedOffset};
use serde_json::Value;

use crate::{sources::source, store};

use super::at_proto::{
    utils::{to_embed, to_record, to_reply, uri_to_post_rkey, uri_to_repost_rkey, AtriumClient},
    Api, Session,
};

pub struct Client {
    api: Api,
    http_client: Arc<reqwest::Client>,
    session: Option<Session>,
    pub identifier: String,
    password: String,
}

impl Client {
    pub fn new(
        origin: String,
        http_client: Arc<reqwest::Client>,
        identifier: String,
        password: String,
    ) -> Self {
        Self {
            api: Api::new(origin),
            http_client,
            session: None,
            identifier,
            password,
        }
    }

    fn as_atrium_client(&self) -> AtriumClient<'_> {
        AtriumClient::new(&self.http_client, &self.session)
    }

    async fn init_session(&mut self) -> Result<()> {
        let session = self
            .api
            .server
            .create_session(&self.http_client, &self.identifier, &self.password)
            .await?;
        self.session = Some(session);
        Ok(())
    }
}

#[async_trait(?Send)]
impl super::Client for Client {
    fn origin(&self) -> &str {
        &self.api.origin
    }

    fn identifier(&self) -> &str {
        &self.identifier
    }

    async fn fetch_statuses(&mut self) -> Result<Vec<source::LiveStatus>> {
        todo!()
    }

    async fn post(
        &mut self,
        content: &str,
        facets: &[store::operations::Facet],
        reply_identifier: Option<&str>,
        images: Vec<store::operations::Medium>,
        external: Option<store::operations::External>,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        let session = match &self.session {
            Some(some) => some,
            None => {
                self.init_session().await?;
                self.session.as_ref().unwrap()
            }
        };
        let reply = to_reply(&self.api, &self.http_client, session, reply_identifier).await?;
        let embed = to_embed(&self.api, &self.http_client, session, images, external).await?;
        let record = to_record(content, facets, reply, embed, created_at);

        let output = self
            .api
            .repo
            .create_record(&self.http_client, session, record)
            .await?;
        Ok(serde_json::to_string(&output)?)
    }

    async fn repost(
        &mut self,
        target_identifier: &str,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        let session = match &self.session {
            Some(some) => some,
            None => {
                self.init_session().await?;
                self.session.as_ref().unwrap()
            }
        };

        let identifier: com::atproto::repo::create_record::Output =
            serde_json::from_str(target_identifier)?;
        let record = atrium_api::records::Record::AppBskyFeedRepost(Box::new(
            app::bsky::feed::repost::Record {
                created_at: created_at.to_rfc3339(),
                subject: com::atproto::repo::strong_ref::Main {
                    cid: identifier.cid,
                    uri: identifier.uri,
                },
            },
        ));
        let res = self
            .as_atrium_client()
            .create_record(com::atproto::repo::create_record::Input {
                collection: String::from("app.bsky.feed.repost"),
                record,
                repo: session.did.clone(),
                rkey: None,
                swap_commit: None,
                validate: None,
            })
            .await
            .map_err(|err| anyhow::anyhow!("{:?}", err))?;
        Ok(serde_json::to_string(&res)?)
    }

    async fn delete_post(&mut self, identifier: &str) -> Result<()> {
        let json: Value = serde_json::from_str(identifier)?;
        let uri = json
            .get("uri")
            .ok_or_else(|| anyhow!("uri not found ({})", identifier))?
            .as_str()
            .ok_or_else(|| anyhow!("uri is not string"))?;
        let rkey = uri_to_post_rkey(uri)?;

        let session = match &self.session {
            Some(some) => some,
            None => {
                self.init_session().await?;
                self.session.as_ref().unwrap()
            }
        };

        self.api
            .repo
            .delete_record(&self.http_client, session, &rkey)
            .await?;
        Ok(())
    }

    async fn delete_repost(&mut self, identifier: &str) -> Result<()> {
        let output: com::atproto::repo::put_record::Output = serde_json::from_str(identifier)?;
        let rkey = uri_to_repost_rkey(&output.uri)?;

        let session = match &self.session {
            Some(some) => some,
            None => {
                self.init_session().await?;
                self.session.as_ref().unwrap()
            }
        };

        let input = com::atproto::repo::delete_record::Input {
            collection: "app.bsky.feed.repost".into(),
            repo: session.did.clone(),
            rkey,
            swap_commit: None,
            swap_record: None,
        };
        self.as_atrium_client()
            .delete_record(input)
            .await
            .map_err(|err| anyhow::anyhow!("{:?}", err))?;

        Ok(())
    }
}
