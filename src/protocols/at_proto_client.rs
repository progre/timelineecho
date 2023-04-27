use anyhow::{anyhow, Result};
use atrium_api::{app::bsky::feed::post::ReplyRef, com::atproto::repo::strong_ref};
use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};

use crate::store::{self, Facet};

use super::at_proto::{
    repo::{Embed, External, Image, Record},
    Api, Session,
};

fn to_record<'a>(
    text: &'a str,
    facets: &'a [store::Facet],
    reply: Option<ReplyRef>,
    embed: Option<&'a Embed>,
    created_at: &'a str,
) -> Record<'a> {
    Record {
        text,
        facets: facets
            .iter()
            .map(|facet| match facet {
                Facet::Mention {
                    byte_slice,
                    identifier,
                } => json!({
                    "index": {
                        "byteStart": byte_slice.start,
                        "byteEnd": byte_slice.end
                    },
                    "features": [{
                        "$type": "app.bsky.richtext.facet#mention",
                        "did": identifier,
                    }]
                }),
                Facet::Link { byte_slice, uri } => json!({
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

pub struct Client {
    api: Api,
    http_client: reqwest::Client,
    session: Option<Session>,
    pub identifier: String,
    password: String,
}

impl Client {
    pub fn new(
        origin: String,
        http_client: reqwest::Client,
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

    pub fn origin(&self) -> &str {
        &self.api.origin
    }

    pub async fn post(
        &mut self,
        content: &str,
        facets: &[store::Facet],
        reply_identifier: Option<&str>,
        images: Vec<store::Medium>,
        external: Option<store::External>,
        created_at: &str,
    ) -> Result<String> {
        let session = match &self.session {
            Some(some) => some,
            None => {
                let session = self
                    .api
                    .server
                    .create_session(&self.http_client, &self.identifier, &self.password)
                    .await?;
                self.session = Some(session);
                self.session.as_ref().unwrap()
            }
        };

        let reply: Option<ReplyRef> = reply_identifier
            .map(|reply_identifier| -> Result<ReplyRef> {
                let parent: strong_ref::Main = serde_json::from_str(reply_identifier)?;
                let root: strong_ref::Main = serde_json::from_str(reply_identifier)?;
                Ok(ReplyRef { parent, root })
            })
            .transpose()?;
        let embed = if !images.is_empty() {
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
            Some(Embed::Images(array))
        } else if let Some(external) = external {
            let resp = self.http_client.get(&external.thumb_url).send().await?;
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

            Some(Embed::External(External {
                uri: external.uri,
                title: external.title,
                description: external.description,
                thumb,
            }))
        } else {
            None
        };

        let record = to_record(content, facets, reply, embed.as_ref(), created_at);

        let output = self
            .api
            .repo
            .create_record(&self.http_client, session, record)
            .await?;
        Ok(serde_json::to_string(&output)?)
    }

    pub async fn delete(&mut self, identifier: &str) -> Result<()> {
        let json: Value = serde_json::from_str(identifier)?;
        let uri = json
            .get("uri")
            .ok_or_else(|| anyhow!("uri not found"))?
            .as_str()
            .ok_or_else(|| anyhow!("uri is not string"))?;
        let rkey = Regex::new(r"at://did:plc:.+?/app.bsky.feed.post/(.+)")
            .unwrap()
            .captures(uri)
            .ok_or_else(|| anyhow!("invalid uri format"))?[1]
            .to_owned();

        let session = match &self.session {
            Some(some) => some,
            None => {
                let session = self
                    .api
                    .server
                    .create_session(&self.http_client, &self.identifier, &self.password)
                    .await?;
                self.session = Some(session);
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
