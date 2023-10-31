use anyhow::{anyhow, Result};
use atrium_api::app::{
    self,
    bsky::feed::defs::{FeedViewPostReasonEnum, PostViewEmbedEnum},
};
use chrono::DateTime;
use regex::Regex;

use crate::{sources::source, store};

impl TryFrom<app::bsky::richtext::facet::Main> for store::operations::Facet {
    type Error = anyhow::Error;

    fn try_from(value: app::bsky::richtext::facet::Main) -> Result<Self> {
        assert_eq!(value.features.len(), 1);
        let feature = &value.features[0];
        match feature {
            app::bsky::richtext::facet::MainFeaturesItem::Mention(mention) => {
                Err(anyhow!("mention is not implemented: {:?}", mention))
            }
            app::bsky::richtext::facet::MainFeaturesItem::Link(link) => {
                Ok(store::operations::Facet::Link {
                    byte_slice: (value.index.byte_start as u32)..(value.index.byte_end as u32),
                    uri: link.uri.clone(),
                })
            }
            app::bsky::richtext::facet::MainFeaturesItem::Tag(tag) => {
                Err(anyhow!("tag is not implemented: {:?}", tag))
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

fn rewrite_content(
    mut content: String,
    mut facets: Option<Vec<app::bsky::richtext::facet::Main>>,
) -> String {
    let Some(facets) = &mut facets else {
        return content;
    };
    facets.sort_by_key(|x| x.index.byte_start);
    facets.reverse();
    for facet in facets {
        let Some(link) = facet
            .features
            .iter()
            .filter_map(|x| match x {
                app::bsky::richtext::facet::MainFeaturesItem::Link(link) => Some(link),
                app::bsky::richtext::facet::MainFeaturesItem::Mention(_) => None,
                app::bsky::richtext::facet::MainFeaturesItem::Tag(_) => None,
            })
            .next()
        else {
            continue;
        };
        content.replace_range(
            (facet.index.byte_start as usize)..(facet.index.byte_end as usize),
            &link.uri,
        );
    }
    content
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
                let facets = record
                    .facets
                    .iter()
                    .flatten()
                    .filter_map(|x| x.to_owned().try_into().ok())
                    .collect();
                source::LiveStatus::Post(source::LivePost {
                    identifier: value.post.cid.clone(),
                    uri: value.post.uri.clone(),
                    content: rewrite_content(record.text.to_owned(), record.facets),
                    facets,
                    reply_src_identifier: record.reply.map(|x| x.parent.cid),
                    media,
                    external,
                    created_at: DateTime::parse_from_rfc3339(&record.created_at)?,
                })
            },
        )
    }
}
