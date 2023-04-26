use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::header::CONTENT_TYPE;

use crate::store::{Facet, Medium};

use super::at_proto::{Api, Session};

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
        facets: &[Facet],
        images: &[Medium],
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
            array.push((
                res.get_mut("blob")
                    .ok_or_else(|| anyhow!("blob not found"))?
                    .take(),
                image.alt.clone(),
            ));
        }
        let res = self
            .api
            .repo
            .create_record(&self.http_client, session, content, facets, &array)
            .await?;
        let uri = res
            .get("uri")
            .ok_or_else(|| anyhow!("uri not found"))?
            .as_str()
            .ok_or_else(|| anyhow!("uri is not string"))?;
        let rid = Regex::new(r"at://did:plc:.+?/app.bsky.feed.post/(.+)")
            .unwrap()
            .captures(uri)
            .ok_or_else(|| anyhow!("invalid uri format"))?[1]
            .to_owned();
        Ok(rid)
    }
}
