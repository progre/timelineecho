use serde::{Deserialize, Serialize};

use crate::{app::AccountKey, sources::source};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub identifier: String,
    pub content: String,
}

impl From<super::operations::CreateOperationStatus> for SourceStatus {
    fn from(full: super::operations::CreateOperationStatus) -> Self {
        SourceStatus {
            identifier: full.src_identifier,
            content: full.content,
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub origin: String,
    pub identifier: String,
    pub statuses: Vec<SourceStatus>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationStatus {
    pub identifier: String,
    pub src_identifier: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub origin: String,
    pub identifier: String,
    pub statuses: Vec<DestinationStatus>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub src: Source,
    pub dsts: Vec<Destination>,
}

impl User {
    pub fn get_or_create_dst_mut<'a>(
        &'a mut self,
        account_key: &AccountKey,
    ) -> &'a mut Destination {
        let idx = self.dsts.iter().position(|dst| {
            dst.origin == account_key.origin && dst.identifier == account_key.identifier
        });
        if let Some(idx) = idx {
            return &mut self.dsts[idx];
        }
        self.dsts.push(Destination {
            origin: account_key.origin.clone(),
            identifier: account_key.identifier.clone(),
            statuses: Vec::default(),
        });
        self.dsts.last_mut().unwrap()
    }
}
