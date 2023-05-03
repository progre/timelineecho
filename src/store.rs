use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::{protocols::Client, sources::source};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub identifier: String,
    pub content: String,
}

impl From<CreatingStatus> for SourceStatus {
    fn from(full: CreatingStatus) -> Self {
        SourceStatus {
            identifier: full.source_status.src_identifier,
            content: full.source_status.content,
        }
    }
}

impl From<source::LiveStatus> for SourceStatus {
    fn from(full: source::LiveStatus) -> Self {
        SourceStatus {
            identifier: full.identifier,
            content: full.content,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub origin: String,
    pub identifier: String,
    pub statuses: Vec<SourceStatus>,
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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationStatus {
    pub identifier: String,
    pub src_identifier: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub origin: String,
    pub identifier: String,
    pub statuses: Vec<DestinationStatus>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub src: Source,
    pub dsts: Vec<Destination>,
}

impl User {
    pub fn find_dst<'a>(&'a self, origin: &str, identifier: &str) -> Option<&'a Destination> {
        self.dsts
            .iter()
            .find(|dst| dst.origin == origin && dst.identifier == identifier)
    }

    pub fn get_or_create_dst<'a>(
        &'a mut self,
        origin: &str,
        identifier: &str,
    ) -> &'a mut Destination {
        let idx = self
            .dsts
            .iter()
            .position(|dst| dst.origin == origin && dst.identifier == identifier);
        if let Some(idx) = idx {
            return &mut self.dsts[idx];
        }
        self.dsts.push(Destination {
            origin: origin.to_owned(),
            identifier: identifier.to_owned(),
            statuses: Vec::default(),
        });
        self.dsts.last_mut().unwrap()
    }
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
    pub fn from_clients(src_client: &dyn Client, dst_client: &dyn Client) -> Self {
        Self {
            src_origin: src_client.origin().to_owned(),
            src_account_identifier: src_client.identifier().to_owned(),
            dst_origin: dst_client.origin().to_owned(),
            dst_account_identifier: dst_client.identifier().to_owned(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatusFull {
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
pub struct CreatingStatus {
    #[serde(flatten)]
    pub account_pair: AccountPair,
    #[serde(flatten)]
    pub source_status: SourceStatusFull,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "operation")]
pub enum Operation {
    #[serde(rename_all = "camelCase")]
    Create(CreatingStatus),
    #[serde(rename_all = "camelCase")]
    Update {
        #[serde(flatten)]
        account_pair: AccountPair,
        dst_identifier: String,
        content: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        facets: Vec<Facet>,
    },
    #[serde(rename_all = "camelCase")]
    Delete {
        #[serde(flatten)]
        account_pair: AccountPair,
        dst_identifier: String,
    },
}

impl Operation {
    pub fn account_pair(&self) -> &AccountPair {
        match self {
            Operation::Create(CreatingStatus {
                account_pair,
                source_status: _,
            })
            | Operation::Update {
                account_pair,
                dst_identifier: _,
                content: _,
                facets: _,
            }
            | Operation::Delete {
                account_pair,
                dst_identifier: _,
            } => account_pair,
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub users: Vec<User>,
    pub operations: Vec<Operation>,
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
        .get_or_create_dst(
            &account_pair.dst_origin,
            &account_pair.dst_account_identifier,
        )
    }

    fn necessary_src_identifiers(&self) -> Vec<String> {
        self.users
            .iter()
            .flat_map(|user| {
                user.src
                    .statuses
                    .iter()
                    .map(|src_status| src_status.identifier.clone())
            })
            .collect()
    }

    pub fn retain_all_dst_statuses(&mut self) {
        let necessary_src_identifiers = self.necessary_src_identifiers();

        for user in &mut self.users {
            for dst in &mut user.dsts {
                dst.statuses
                    .retain(|status| necessary_src_identifiers.contains(&status.src_identifier));
            }
        }
    }
}
