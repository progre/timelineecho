pub mod user;

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::app::AccountKey;

use self::user::{Destination, Source, User};

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
pub struct CreatingStatus {
    pub src_identifier: String,
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
    pub created_at: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "operation")]
pub enum Operation {
    #[serde(rename_all = "camelCase")]
    Create(CreatingStatus),
    #[serde(rename_all = "camelCase")]
    Update {
        dst_identifier: String,
        content: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        facets: Vec<Facet>,
    },
    #[serde(rename_all = "camelCase")]
    Delete { dst_identifier: String },
}

#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPair {
    pub src_origin: String,
    pub src_account_identifier: String,
    pub dst_origin: String,
    pub dst_account_identifier: String,
}

impl AccountPair {
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

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub users: Vec<User>,
}

impl Store {
    pub fn get_or_create_user<'a>(&'a mut self, origin: &str, identifier: &str) -> &'a mut User {
        let idx = self
            .users
            .iter()
            .position(|user| user.src.origin == origin && user.src.identifier == identifier);
        if let Some(idx) = idx {
            return &mut self.users[idx];
        }
        self.users.push(User {
            src: Source {
                origin: origin.to_owned(),
                identifier: identifier.to_owned(),
                statuses: Vec::default(),
            },
            dsts: Vec::default(),
        });
        self.users.last_mut().unwrap()
    }

    pub fn get_or_create_dst<'a>(&'a mut self, account_pair: &AccountPair) -> &'a mut Destination {
        self.get_or_create_user(
            &account_pair.src_origin,
            &account_pair.src_account_identifier,
        )
        .get_or_create_dst(&account_pair.to_dst_key())
    }
}
