use anyhow::Result;
use chrono::Utc;
use reqwest::{header::CONTENT_TYPE, Body};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::error;

use crate::store::{
    self,
    Facet::{Link, Mention},
};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    did: String,
    handle: String,
    email: String,
    access_jwt: String,
    refresh_jwt: String,
}

pub struct Api {
    pub origin: String,
}

impl Api {
    pub fn new(origin: String) -> Self {
        Self { origin }
    }

    async fn _query<T: DeserializeOwned, U: Serialize + ?Sized>(
        &self,
        client: &reqwest::Client,
        token: &str,
        lexicon_id: &str,
        query: &U,
    ) -> Result<T> {
        let resp = client
            .get(format!("{}/xrpc/{}", self.origin, lexicon_id))
            .query(query)
            .bearer_auth(token)
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
        Ok(resp.json().await?)
    }

    async fn procedure<T: DeserializeOwned>(
        &self,
        client: &reqwest::Client,
        token: &str,
        lexicon_id: &str,
        properties: &Value,
    ) -> Result<T> {
        let resp = client
            .post(format!("{}/xrpc/{}", self.origin, lexicon_id))
            .bearer_auth(token)
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
        Ok(resp.json().await?)
    }

    pub async fn create_session(
        &self,
        client: &reqwest::Client,
        identifier: &str,
        password: &str,
    ) -> Result<Session> {
        let lexicon_id = "com.atproto.server.createSession";
        let properties = &json!({
            "identifier": identifier,
            "password": password
        });
        Ok(client
            .post(format!("{}/xrpc/{}", self.origin, lexicon_id))
            .json(properties)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn _refresh_session(
        &self,
        client: &reqwest::Client,
        session: &Session,
    ) -> Result<Value> {
        let lexicon_id = "com.atproto.server.refreshSession";
        Ok(client
            .post(format!("{}/xrpc/{}", self.origin, lexicon_id))
            .bearer_auth(&session.refresh_jwt)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
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
        self.procedure(
            client,
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
        let lexicon_id = "com.atproto.repo.getRecord";
        let query = &[
            ("repo", session.did.as_str()),
            ("collection", "app.bsky.feed.post"),
            ("rkey", rkey),
        ];

        self._query(client, &session.access_jwt, lexicon_id, query)
            .await
    }

    pub async fn _list_records(
        &self,
        client: &reqwest::Client,
        session: &Session,
    ) -> Result<Value> {
        let lexicon_id = "com.atproto.repo.listRecords";
        let query = &[
            ("repo", session.did.as_str()),
            ("collection", "app.bsky.feed.post"),
        ];

        self._query(client, &session.access_jwt, lexicon_id, query)
            .await
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
