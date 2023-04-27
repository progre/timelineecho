use anyhow::Result;
use atrium_api::app::bsky::feed::post::ReplyRef;
use reqwest::{header::CONTENT_TYPE, Body};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::error;

use crate::protocols::at_proto::procedure;

use super::{query, Session};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub thumb: Value,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub image: Value,
    pub alt: String,
}

pub enum Embed {
    External(External),
    Images(Vec<Image>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record<'a> {
    pub text: &'a str,
    pub facets: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<ReplyRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<Value>,
    pub created_at: &'a str,
}

pub struct Repo {
    origin: String,
}

impl Repo {
    pub fn new(origin: String) -> Self {
        Self { origin }
    }

    pub async fn create_record(
        &self,
        client: &reqwest::Client,
        session: &Session,
        record: Record<'_>,
    ) -> Result<Value> {
        let lexicon_id = "com.atproto.repo.createRecord";
        procedure(
            client,
            &self.origin,
            &session.access_jwt,
            lexicon_id,
            &json!({
                "repo": &session.did,
                "collection": "app.bsky.feed.post",
                "record": &record,
            }),
        )
        .await
    }

    pub async fn delete_record(
        &self,
        client: &reqwest::Client,
        session: &Session,
        rkey: &str,
    ) -> Result<()> {
        let lexicon_id = "com.atproto.repo.deleteRecord";
        let properties = &json!({
            "repo": &session.did,
            "collection": "app.bsky.feed.post",
            "rkey": rkey
        });

        let resp = client
            .post(format!("{}/xrpc/{}", &self.origin, lexicon_id))
            .bearer_auth(&session.access_jwt)
            .json(properties)
            .send()
            .await?;
        if let Err(err) = resp.error_for_status_ref() {
            let json: Value = resp.json().await?;
            error!(
                "url={:?}, status-code={:?}, body={}",
                err.url().map(ToString::to_string),
                err.status(),
                json
            );
            return Err(err.into());
        }
        // NOTE: 空文字が返る
        Ok(())
    }

    #[allow(unused)]
    pub async fn get_record(
        &self,
        client: &reqwest::Client,
        session: &Session,
        rkey: &str,
    ) -> Result<Value> {
        let token = &session.access_jwt;
        let lexicon_id = "com.atproto.repo.getRecord";
        let query_params = &[
            ("repo", session.did.as_str()),
            ("collection", "app.bsky.feed.post"),
            ("rkey", rkey),
        ];

        query(client, &self.origin, token, lexicon_id, query_params).await
    }

    #[allow(unused)]
    pub async fn list_records(&self, client: &reqwest::Client, session: &Session) -> Result<Value> {
        let token = &session.access_jwt;
        let lexicon_id = "com.atproto.repo.listRecords";
        let query_params = &[
            ("repo", session.did.as_str()),
            ("collection", "app.bsky.feed.post"),
        ];

        query(client, &self.origin, token, lexicon_id, query_params).await
    }

    pub async fn upload_blob(
        &self,
        client: &reqwest::Client,
        session: &Session,
        content_type: String,
        body: impl Into<Body>,
    ) -> Result<Value> {
        let lexicon_id = "com.atproto.repo.uploadBlob";
        Ok(client
            .post(format!("{}/xrpc/{}", self.origin, lexicon_id))
            .bearer_auth(&session.access_jwt)
            .header(CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
        // {
        //     "blob": {
        //         "$type": "blob",
        //         "mimeType": "image/jpeg",
        //         "ref": {
        //             "$link": "bafkreihkqppell6jipqwq2izfcleeft5oqzurzx6fplwtwvf4oub5zdnye"
        //         },
        //         "size": 781895
        //     }
        // }
    }
}
