use anyhow::{anyhow, Result};
use atrium_api::app::{
    self,
    bsky::{
        embed::{record::ViewRecordEnum, record_with_media::ViewMediaEnum},
        feed::defs::{FeedViewPostReasonEnum, PostViewEmbedEnum},
    },
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

fn to_external_uri(at_uri: &str) -> String {
    let m = Regex::new(r"^at://(.+?)/app.bsky.feed.post/(.+?)$")
        .unwrap()
        .captures(at_uri)
        .unwrap();
    format!(
        "https://bsky.app/profile/{}/post/{}",
        m.get(1).unwrap().as_str(),
        m.get(2).unwrap().as_str(),
    )
}

fn rewrite_content(
    mut content: String,
    mut facets: Option<Vec<app::bsky::richtext::facet::Main>>,
    quote: Option<&str>,
) -> String {
    if let Some(facets) = &mut facets {
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
    }
    if let Some(quote) = quote {
        if !content.contains(quote) {
            content.push_str("\n\n");
            content.push_str(quote);
        }
    }
    content
}

fn parse_embed(
    embed: Option<PostViewEmbedEnum>,
) -> (
    Vec<store::operations::Medium>,
    source::LiveExternal,
    Option<String>,
) {
    match embed {
        Some(PostViewEmbedEnum::AppBskyEmbedImagesView(images)) => (
            images.images.into_iter().map(|x| x.into()).collect(),
            source::LiveExternal::None,
            None,
        ),
        Some(PostViewEmbedEnum::AppBskyEmbedExternalView(external)) => {
            (vec![], external.into(), None)
        }
        Some(PostViewEmbedEnum::AppBskyEmbedRecordView(embed)) => match embed.record {
            ViewRecordEnum::ViewRecord(record) => (
                vec![],
                source::LiveExternal::None,
                Some(to_external_uri(&record.uri)),
            ),
            ViewRecordEnum::ViewNotFound(_)
            | ViewRecordEnum::ViewBlocked(_)
            | ViewRecordEnum::AppBskyFeedDefsGeneratorView(_)
            | ViewRecordEnum::AppBskyGraphDefsListView(_) => {
                (vec![], source::LiveExternal::None, None)
            }
        },
        Some(PostViewEmbedEnum::AppBskyEmbedRecordWithMediaView(embed)) => {
            let (media, external) = match embed.media {
                ViewMediaEnum::AppBskyEmbedImagesView(images) => (
                    images.images.into_iter().map(|x| x.into()).collect(),
                    source::LiveExternal::None,
                ),
                ViewMediaEnum::AppBskyEmbedExternalView(external) => (vec![], external.into()),
            };
            match embed.record.record {
                ViewRecordEnum::ViewRecord(record) => {
                    (media, external, Some(to_external_uri(&record.uri)))
                }
                ViewRecordEnum::ViewNotFound(_)
                | ViewRecordEnum::ViewBlocked(_)
                | ViewRecordEnum::AppBskyFeedDefsGeneratorView(_)
                | ViewRecordEnum::AppBskyGraphDefsListView(_) => {
                    (vec![], source::LiveExternal::None, None)
                }
            }
        }
        None => (vec![], source::LiveExternal::None, None),
    }
}

impl TryFrom<app::bsky::feed::defs::FeedViewPost> for source::LiveStatus {
    type Error = anyhow::Error;

    fn try_from(value: app::bsky::feed::defs::FeedViewPost) -> Result<Self> {
        let atrium_api::records::Record::AppBskyFeedPost(record) = value.post.record else {
            unreachable!()
        };
        let (media, external, quote) = parse_embed(value.post.embed);
        Ok(
            if let Some(FeedViewPostReasonEnum::ReasonRepost(reason)) = value.reason {
                source::LiveStatus::Repost(store::operations::CreateRepostOperationStatus {
                    src_identifier: value.post.cid.clone(),
                    target_src_identifier: value.post.cid.clone(),
                    target_src_uri: to_external_uri(&value.post.uri),
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
                    content: rewrite_content(
                        record.text.to_owned(),
                        record.facets,
                        quote.as_deref(),
                    ),
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
