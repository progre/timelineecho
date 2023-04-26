use anyhow::Result;
use serde_json::{json, Value};

use super::Session;

pub struct Server {
    origin: String,
}

impl Server {
    pub fn new(origin: String) -> Self {
        Self { origin }
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
}
