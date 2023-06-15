use anyhow::Result;
use atrium_api::app::{
    self,
    bsky::feed::defs::{FeedViewPostReasonEnum, PostViewEmbedEnum},
};
use chrono::DateTime;
use regex::Regex;

use crate::{sources::source, store};

impl From<app::bsky::richtext::facet::Main> for store::operations::Facet {
    fn from(value: app::bsky::richtext::facet::Main) -> Self {
        assert_eq!(value.features.len(), 1);
        let feature = &value.features[0];
        match feature {
            app::bsky::richtext::facet::MainFeaturesItem::Mention(_) => todo!(),
            app::bsky::richtext::facet::MainFeaturesItem::Link(link) => {
                store::operations::Facet::Link {
                    byte_slice: (value.index.byte_start as u32)..(value.index.byte_end as u32),
                    uri: link.uri.clone(),
                }
            }
        }
    }
}

impl From<app::bsky::embed::images::ViewImage> for store::operations::Medium {
    fn from(value: app::bsky::embed::images::ViewImage) -> Self {
        store::operations::Medium {
            alt: value.alt,
            url: value.fullsize,
        }
    }
}

impl From<Box<app::bsky::embed::external::View>> for source::LiveExternal {
    fn from(value: Box<app::bsky::embed::external::View>) -> Self {
        source::LiveExternal::Some(store::operations::External {
            uri: value.external.uri.clone(),
            title: value.external.title.clone(),
            description: value.external.description.clone(),
            thumb_url: value.external.thumb,
        })
    }
}

impl TryFrom<app::bsky::feed::defs::FeedViewPost> for source::LiveStatus {
    type Error = anyhow::Error;

    fn try_from(value: app::bsky::feed::defs::FeedViewPost) -> Result<Self> {
        let atrium_api::records::Record::AppBskyFeedPost(record) = value.post.record else {
            unreachable!()
        };
        let (media, external) = match value.post.embed {
            Some(PostViewEmbedEnum::AppBskyEmbedImagesView(images)) => (
                images.images.into_iter().map(|x| x.into()).collect(),
                source::LiveExternal::None,
            ),
            Some(PostViewEmbedEnum::AppBskyEmbedExternalView(external)) => {
                (vec![], external.into())
            }
            Some(PostViewEmbedEnum::AppBskyEmbedRecordView(_))
            | Some(PostViewEmbedEnum::AppBskyEmbedRecordWithMediaView(_))
            | None => (vec![], source::LiveExternal::None),
        };
        Ok(
            if let Some(FeedViewPostReasonEnum::ReasonRepost(reason)) = value.reason {
                let m = Regex::new(r"^at://(.+?)/app.bsky.feed.post/(.+?)$")
                    .unwrap()
                    .captures(&value.post.uri)
                    .unwrap();
                source::LiveStatus::Repost(store::operations::CreateRepostOperationStatus {
                    src_identifier: value.post.cid.clone(),
                    target_src_identifier: value.post.cid.clone(),
                    target_src_uri: format!(
                        "https://bsky.app/profile/{}/post/{}",
                        m.get(1).unwrap().as_str(),
                        m.get(2).unwrap().as_str(),
                    ),
                    created_at: DateTime::parse_from_rfc3339(&reason.indexed_at)?,
                })
            } else {
                source::LiveStatus::Post(source::LivePost {
                    identifier: value.post.cid.clone(),
                    uri: value.post.uri.clone(),
                    content: record.text,
                    facets: record
                        .facets
                        .into_iter()
                        .flatten()
                        .map(|x| x.into())
                        .collect(),
                    reply_src_identifier: record.reply.map(|x| x.parent.cid),
                    media,
                    external,
                    created_at: DateTime::parse_from_rfc3339(&record.created_at)?,
                })
            },
        )
    }
}
