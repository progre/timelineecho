use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use atrium_api::{
    app,
    com::{self, atproto::repo::create_record::CreateRecord},
};
use chrono::{DateTime, FixedOffset};
use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};

use crate::{
    sources::source,
    store::{self, operations::Facet::Link},
};

use super::at_proto::{
    repo::{Embed, External, Image, Record},
    Api, Session,
};

fn to_record<'a>(
    text: &'a str,
    facets: &'a [store::operations::Facet],
    reply: Option<app::bsky::feed::post::ReplyRef>,
    embed: Option<&'a Embed>,
    created_at: &'a DateTime<FixedOffset>,
) -> Record<'a> {
    Record {
        text,
        facets: facets
            .iter()
            .map(|facet| match facet {
                // NOTE: 実装予定なし
                // Mention {
                //     byte_slice,
                //     src_identifier,
                // } => {
                //     json!({
                //         "index": {
                //             "byteStart": byte_slice.start,
                //             "byteEnd": byte_slice.end
                //         },
                //         "features": [{
                //             "$type": "app.bsky.richtext.facet#mention",
                //             "did": "TODO",
                //         }]
                //     })
                // }
                Link { byte_slice, uri } => json!({
                    "index": {
                        "byteStart": byte_slice.start,
                        "byteEnd": byte_slice.end
                    },
                    "features": [{
                        "$type": "app.bsky.richtext.facet#link",
                        "uri": uri,
                    }]
                }),
            })
            .collect::<Vec<_>>(),
        reply,
        embed: embed.map(|embed| match embed {
            Embed::External(external) => json!({
                "$type": "app.bsky.embed.external",
                "external": external,
            }),
            Embed::Images(images) => json!({
                "$type": "app.bsky.embed.images",
                "images": images,
            }),
        }),
        created_at,
    }
}

fn uri_to_rkey(uri: &str) -> Result<String> {
    Ok(Regex::new(r"at://did:plc:.+?/app.bsky.feed.post/(.+)")
        .unwrap()
        .captures(uri)
        .ok_or_else(|| anyhow!("invalid uri format"))?[1]
        .to_owned())
}

struct AtriumClient<'a> {
    http_client: &'a reqwest::Client,
    session: &'a Option<Session>,
}

#[async_trait::async_trait]
impl atrium_api::xrpc::HttpClient for AtriumClient<'_> {
    async fn send(
        &self,
        req: http::Request<Vec<u8>>,
    ) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
        let resp = self.http_client.execute(req.try_into()?).await?;
        let mut builder = http::Response::builder().status(resp.status());
        for (k, v) in resp.headers() {
            builder = builder.header(k, v);
        }
        builder
            .body(resp.bytes().await?.to_vec())
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl atrium_api::xrpc::XrpcClient for AtriumClient<'_> {
    fn host(&self) -> &str {
        "https://bsky.social"
    }
    fn auth(&self) -> Option<&str> {
        self.session
            .as_ref()
            .map(|session| session.access_jwt.as_str())
    }
}

atrium_api::impl_traits!(AtriumClient<'_>);

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
        AtriumClient {
            http_client: &self.http_client,
            session: &self.session,
        }
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

    async fn to_embed(
        &self,
        session: &Session,
        images: Vec<store::operations::Medium>,
        external: Option<store::operations::External>,
    ) -> Result<Option<Embed>> {
        if !images.is_empty() {
            let mut array = Vec::new();
            for image in images {
                let resp = self.http_client.get(&image.url).send().await?;
                let content_type = resp
                    .headers()
                    .get(CONTENT_TYPE)
                    .ok_or_else(|| anyhow!("no content-type"))?
                    .to_str()?
                    .to_owned();

                let mut res = self
                    .api
                    .repo
                    .upload_blob(&self.http_client, session, content_type, resp)
                    .await?;
                let alt = image.alt;
                let image = res
                    .get_mut("blob")
                    .ok_or_else(|| anyhow!("blob not found"))?
                    .take();
                array.push(Image { image, alt });
            }
            return Ok(Some(Embed::Images(array)));
        }
        if let Some(external) = external {
            if let Some(thumb_url) = &external.thumb_url {
                let resp = self.http_client.get(thumb_url).send().await?;
                let content_type = resp
                    .headers()
                    .get(CONTENT_TYPE)
                    .ok_or_else(|| anyhow!("no content-type"))?
                    .to_str()?
                    .to_owned();

                let mut res = self
                    .api
                    .repo
                    .upload_blob(&self.http_client, session, content_type, resp)
                    .await?;
                let thumb = res
                    .get_mut("blob")
                    .ok_or_else(|| anyhow!("blob not found"))?
                    .take();
                return Ok(Some(Embed::External(External {
                    uri: external.uri,
                    title: external.title,
                    description: external.description,
                    thumb,
                })));
            }
        }
        Ok(None)
    }

    async fn find_reply_root(
        &self,
        session: &Session,
        rkey: &str,
    ) -> Result<Option<com::atproto::repo::strong_ref::Main>> {
        let record = self
            .api
            .repo
            .get_record(&self.http_client, session, rkey)
            .await?;
        let atrium_api::records::Record::AppBskyFeedPost(record) = record.value else {
            unreachable!();
        };
        let Some(reply) = record.reply else {
            return Ok(None);
        };

        Ok(Some(reply.root))
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
        let reply = if let Some(reply_identifier) = reply_identifier {
            let parent: com::atproto::repo::strong_ref::Main =
                serde_json::from_str(reply_identifier)?;
            let root = self
                .find_reply_root(session, &uri_to_rkey(&parent.uri)?)
                .await?
                .unwrap_or_else(|| parent.clone());
            Some(app::bsky::feed::post::ReplyRef { parent, root })
        } else {
            None
        };

        let embed = self.to_embed(session, images, external).await?;
        let record = to_record(content, facets, reply, embed.as_ref(), created_at);

        let output = self
            .api
            .repo
            .create_record(&self.http_client, session, record)
            .await?;
        Ok(serde_json::to_string(&output)?)
    }

    async fn repost(
        &mut self,
        identifier: &str,
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
            serde_json::from_str(identifier)?;
        let record =
            atrium_api::records::Record::AppBskyFeedRepost(app::bsky::feed::repost::Record {
                created_at: created_at.to_rfc3339(),
                subject: com::atproto::repo::strong_ref::Main {
                    cid: identifier.cid,
                    uri: identifier.uri,
                },
            });
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

    async fn delete(&mut self, identifier: &str) -> Result<()> {
        let json: Value = serde_json::from_str(identifier)?;
        let uri = json
            .get("uri")
            .ok_or_else(|| anyhow!("uri not found"))?
            .as_str()
            .ok_or_else(|| anyhow!("uri is not string"))?;
        let rkey = uri_to_rkey(uri)?;

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
}
