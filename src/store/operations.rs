use std::ops::Range;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::{app::AccountKey, utils::format_rfc3339};

#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPair {
    pub src_origin: String,
    pub src_account_identifier: String,
    pub dst_origin: String,
    pub dst_account_identifier: String,
}

impl AccountPair {
    pub fn from_keys(src_account_key: AccountKey, dst_account_key: AccountKey) -> Self {
        Self {
            src_origin: src_account_key.origin,
            src_account_identifier: src_account_key.identifier,
            dst_origin: dst_account_key.origin,
            dst_account_identifier: dst_account_key.identifier,
        }
    }

    pub fn to_src_key(&self) -> AccountKey {
        AccountKey {
            origin: self.src_origin.clone(),
            identifier: self.src_account_identifier.clone(),
        }
    }

    pub fn to_dst_key(&self) -> AccountKey {
        AccountKey {
            origin: self.dst_origin.clone(),
            identifier: self.dst_account_identifier.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum Facet {
    // NOTE: 実装予定なし
    // #[serde(rename_all = "camelCase")]
    // Mention {
    //     byte_slice: Range<u32>,
    //     src_identifier: String,
    // },
    #[serde(rename_all = "camelCase")]
    Link { byte_slice: Range<u32>, uri: String },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Medium {
    pub url: String,
    pub alt: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub thumb_url: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostOperationStatus {
    pub src_identifier: String,
    pub src_uri: String,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub facets: Vec<Facet>,
    pub reply_src_identifier: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub media: Vec<Medium>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<External>,
    #[serde(with = "format_rfc3339")]
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CreatePostOperation {
    #[serde(flatten)]
    pub account_pair: AccountPair,
    #[serde(flatten)]
    pub status: CreatePostOperationStatus,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepostOperationStatus {
    pub src_identifier: String,
    pub target_src_identifier: String,
    pub target_src_uri: String,
    #[serde(with = "format_rfc3339")]
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepostOperation {
    #[serde(flatten)]
    pub account_pair: AccountPair,
    #[serde(flatten)]
    pub status: CreateRepostOperationStatus,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePostOperationStatus {
    pub src_identifier: String,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<Facet>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePostOperation {
    #[serde(flatten)]
    pub account_pair: AccountPair,
    #[serde(flatten)]
    pub status: UpdatePostOperationStatus,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePostOperationStatus {
    pub src_identifier: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePostOperation {
    #[serde(flatten)]
    pub account_pair: AccountPair,
    #[serde(flatten)]
    pub status: DeletePostOperationStatus,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRepostOperationStatus {
    pub src_identifier: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRepostOperation {
    #[serde(flatten)]
    pub account_pair: AccountPair,
    #[serde(flatten)]
    pub status: DeleteRepostOperationStatus,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "operation")]
pub enum Operation {
    CreatePost(CreatePostOperation),
    CreateRepost(CreateRepostOperation),
    UpdatePost(UpdatePostOperation),
    DeletePost(DeletePostOperation),
    DeleteRepost(DeleteRepostOperation),
}

impl Operation {
    pub fn account_pair(&self) -> &AccountPair {
        match self {
            Operation::CreatePost(CreatePostOperation { account_pair, .. })
            | Operation::CreateRepost(CreateRepostOperation { account_pair, .. })
            | Operation::UpdatePost(UpdatePostOperation { account_pair, .. })
            | Operation::DeletePost(DeletePostOperation { account_pair, .. })
            | Operation::DeleteRepost(DeleteRepostOperation { account_pair, .. }) => account_pair,
        }
    }
}
