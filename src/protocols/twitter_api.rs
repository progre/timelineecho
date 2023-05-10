use std::sync::Arc;

use anyhow::Result;
use chrono::NaiveDateTime;
use oauth1_request::{Credentials, ParameterList};
use reqwest::{
    header::{ACCEPT, AUTHORIZATION},
    multipart::{Form, Part},
    Body, Response,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tracing::{error, event_enabled, trace, Level};

async fn trace_header_and_throw_if_error_status(resp: Response) -> Result<Response> {
    let err = resp.error_for_status_ref().err();
    if let Some(err) = err {
        error!("{:?}", resp.text().await?);
        return Err(err.into());
    }
    if !event_enabled!(Level::TRACE) {
        return Ok(resp);
    }
    resp.headers()
        .iter()
        .filter(|(key, _)| {
            [
                "date",
                "x-rate-limit-limit",
                "x-rate-limit-reset",
                "x-rate-limit-remaining",
            ]
            .contains(&key.as_str())
        })
        .for_each(|(key, value)| {
            let value = value.to_str().unwrap_or_default();
            let value = if key == "x-rate-limit-reset" {
                NaiveDateTime::from_timestamp_opt(value.parse::<i64>().unwrap_or_default(), 0)
                    .unwrap_or_default()
                    .to_string()
            } else {
                value.to_owned()
            };
            trace!("{}: {}", key, value);
        });
    Ok(resp)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweetBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<Value>,
    pub text: &'a str,
}

pub struct Api {
    http_client: Arc<reqwest::Client>,
    oauth1_request_builder: oauth1_request::Builder<'static, oauth1_request::HmacSha1>,
}

impl Api {
    pub fn new(
        http_client: Arc<reqwest::Client>,
        api_key: String,
        api_key_secret: String,
        access_token: String,
        access_token_secret: String,
    ) -> Self {
        Self {
            http_client,
            oauth1_request_builder: oauth1_request::Builder::<_, _>::new(
                Credentials {
                    identifier: api_key,
                    secret: api_key_secret,
                },
                oauth1_request::HMAC_SHA1,
            )
            .token(Credentials {
                identifier: access_token,
                secret: access_token_secret,
            })
            .clone(),
        }
    }

    pub async fn _get_me<T: DeserializeOwned>(&self) -> Result<T> {
        let url = "https://api.twitter.com/2/users/me";
        let resp = self
            .http_client
            .get(url)
            .header(AUTHORIZATION, self.oauth1_request_builder.get(url, &()))
            .send()
            .await?;
        let resp = trace_header_and_throw_if_error_status(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn create_tweet<T: DeserializeOwned>(&self, body: TweetBody<'_>) -> Result<T> {
        let url = "https://api.twitter.com/2/tweets";
        let resp = self
            .http_client
            .post(url)
            .header(AUTHORIZATION, self.oauth1_request_builder.post(url, &()))
            .json(&body)
            .send()
            .await?;
        let resp = trace_header_and_throw_if_error_status(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn delete_tweet<T: DeserializeOwned>(&self, id: &str) -> Result<T> {
        let url = format!("https://api.twitter.com/2/tweets/{}", id);
        let resp = self
            .http_client
            .delete(&url)
            .header(AUTHORIZATION, self.oauth1_request_builder.delete(url, &()))
            .header(ACCEPT, "application/json")
            .send()
            .await?;
        let resp = trace_header_and_throw_if_error_status(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn create_retweet<T: DeserializeOwned>(
        &self,
        user_id: &str,
        tweet_id: &str,
    ) -> Result<T> {
        let url = format!("https://api.twitter.com/2/users/{}/retweets", user_id);
        let resp = self
            .http_client
            .post(&url)
            .header(AUTHORIZATION, self.oauth1_request_builder.post(url, &()))
            .json(&json!({ "tweet_id": tweet_id }))
            .send()
            .await?;
        let resp = trace_header_and_throw_if_error_status(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn verify_credentials<T: DeserializeOwned>(&self) -> Result<T> {
        let url = "https://api.twitter.com/1.1/account/verify_credentials.json";
        let resp = self
            .http_client
            .get(url)
            .header(AUTHORIZATION, self.oauth1_request_builder.get(url, &()))
            .send()
            .await?;
        let resp = trace_header_and_throw_if_error_status(resp).await?;
        Ok(resp.json().await?)
    }

    pub async fn upload<T: DeserializeOwned>(&self, body: impl Into<Body>) -> Result<T> {
        let url = "https://upload.twitter.com/1.1/media/upload.json";
        let query = [("media_category", "tweet_image")];
        let multipart = Form::new().part("media", Part::stream(body));

        let resp = self
            .http_client
            .post(url)
            .header(
                AUTHORIZATION,
                self.oauth1_request_builder
                    .post(url, &ParameterList::new(query)),
            )
            .query(&query)
            .multipart(multipart)
            .send()
            .await?;
        let resp = trace_header_and_throw_if_error_status(resp).await?;
        Ok(resp.json().await?)
    }
}
