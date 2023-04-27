use std::ops::Range;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub identifier: String,
    pub content: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub origin: String,
    pub identifier: String,
    pub statuses: Vec<SourceStatus>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationStatus {
    pub identifier: String,
    pub src_identifier: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum Facet {
    #[serde(rename_all = "camelCase")]
    Mention {
        byte_slice: Range<u32>,
        identifier: String,
    },
    #[serde(rename_all = "camelCase")]
    Link { byte_slice: Range<u32>, uri: String },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Medium {
    pub url: String,
    pub alt: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub thumb_url: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "operation")]
pub enum Operation {
    #[serde(rename_all = "camelCase")]
    Create {
        src_status_identifier: String,
        content: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        #[serde(default)]
        facets: Vec<Facet>,
        reply_src_status_identifier: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        #[serde(default)]
        media: Vec<Medium>,
        #[serde(skip_serializing_if = "Option::is_none")]
        external: Option<External>,
        created_at: String,
    },
    #[serde(rename_all = "camelCase")]
    Update {
        src_status_identifier: String,
        content: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        facets: Vec<Facet>,
    },
    #[serde(rename_all = "camelCase")]
    Delete { identifier: String },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub origin: String,
    pub identifier: String,
    pub statuses: Vec<DestinationStatus>,
    pub operations: Vec<Operation>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub src: Source,
    pub dsts: Vec<Destination>,
}

impl User {
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
            operations: Vec::default(),
        });
        self.dsts.last_mut().unwrap()
    }
}

#[derive(Default, Deserialize, Serialize)]
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
}
