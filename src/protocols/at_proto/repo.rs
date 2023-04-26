use anyhow::Result;
use chrono::Utc;
use reqwest::{header::CONTENT_TYPE, Body};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    protocols::at_proto::procedure,
    store::{
        self,
        Facet::{Link, Mention},
    },
};

use super::{query, Session};

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
        text: &str,
        facets: &[store::Facet],
        images: &[(Value, String)],
    ) -> Result<Value> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Record<'a> {
            text: &'a str,
            facets: &'a Vec<Value>,
            #[serde(skip_serializing_if = "Option::is_none")]
            embed: Option<&'a Value>,
            created_at: String,
        }

        let lexicon_id = "com.atproto.repo.createRecord";
        let facets = facets
            .iter()
            .map(|facet| match facet {
                Mention {
                    byte_slice: _,
                    identifier: _,
                } => todo!(),
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
            .collect::<Vec<_>>();
        let embed = if images.is_empty() {
            None
        } else {
            Some(json!({
                "$type": "app.bsky.embed.images",
                "images": images.iter().map(|(image, alt)| json!({"image": image, "alt": alt})).collect::<Vec<_>>(),
            }))
        };
        procedure(
            client,
            &self.origin,
            &session.access_jwt,
            lexicon_id,
            &json!({
                "repo": &session.did,
                "collection": "app.bsky.feed.post",
                "record": Record {
                  text,
                  facets: &facets,
                  embed: embed.as_ref(),
                  created_at: Utc::now().to_rfc3339(),
                }
            }),
        )
        .await
    }

    pub async fn _get_record(
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

    pub async fn _list_records(
        &self,
        client: &reqwest::Client,
        session: &Session,
    ) -> Result<Value> {
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