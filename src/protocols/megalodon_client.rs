use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use html2text::render::text_renderer::RichAnnotation;
use megalodon::{megalodon::GetAccountStatusesInputOptions, Megalodon};
use reqwest::header::HeaderMap;
use tracing::{event_enabled, trace, Level};

use crate::{sources::source, store};

fn trace_header(header: &HeaderMap) {
    if !event_enabled!(Level::TRACE) {
        return;
    }
    header
        .iter()
        .filter(|(key, _)| {
            [
                "date",
                "x-ratelimit-limit",
                "x-ratelimit-remaining",
                "x-ratelimit-reset",
            ]
            .contains(&key.as_str())
        })
        .for_each(|(key, value)| {
            trace!("{}: {}", key, value.to_str().unwrap_or_default());
        });
}

fn link(current_idx: usize, uri: &str) -> store::operations::Facet {
    store::operations::Facet::Link {
        byte_slice: (current_idx as u32)..(current_idx as u32) + (uri.as_bytes().len() as u32),
        uri: uri.to_owned(),
    }
}

fn html_to_content_facets(html: &str) -> (String, Vec<store::operations::Facet>) {
    let content = html2text::from_read_rich(html.as_bytes(), usize::MAX);
    let mut text = String::new();
    let mut facets = Vec::new();
    for line in content {
        for string in line.tagged_strings() {
            if string.tag.is_empty() {
                text += &string.s;
                continue;
            }
            assert_eq!(string.tag.len(), 1);
            if let RichAnnotation::Link(_) = &string.tag[0] {
                // NOTE: ハッシュタグは未対応
                if !string.s.starts_with('#') {
                    facets.push(link(text.as_bytes().len(), &string.s));
                }
                text += &string.s;
                continue;
            }
            unreachable!();
        }
        text += "\n";
    }
    (text.trim_end().to_owned(), facets)
}

pub struct Client {
    origin: String,
    megalodon: Box<dyn Megalodon>,
    account_id: String,
}

impl Client {
    pub async fn new_mastodon(origin: String, access_token: String) -> Result<Self> {
        let megalodon = megalodon::generator(
            megalodon::SNS::Mastodon,
            origin.clone(),
            Some(access_token),
            None,
        );
        let resp = megalodon.verify_account_credentials().await?;
        trace_header(&resp.header);
        let account_id = resp.json().id;

        Ok(Self {
            origin,
            megalodon,
            account_id,
        })
    }
}

#[async_trait(?Send)]
impl super::Client for Client {
    fn origin(&self) -> &str {
        &self.origin
    }

    fn identifier(&self) -> &str {
        &self.account_id
    }

    async fn fetch_statuses(&mut self) -> Result<Vec<source::LiveStatus>> {
        let resp = self
            .megalodon
            .get_account_statuses(
                self.account_id.clone(),
                Some(&GetAccountStatusesInputOptions {
                    limit: Some(40),
                    // exclude_replies: Some(true), // TODO: include self replies
                    ..Default::default()
                }),
            )
            .await?;
        trace_header(&resp.header);
        let statuses: Vec<_> = resp
            .json()
            .into_iter()
            .map(|status| {
                let (content, facets) = html_to_content_facets(&status.content);
                source::LiveStatus {
                    identifier: status.id,
                    content,
                    facets,
                    reply_src_identifier: status.in_reply_to_id,
                    media: status
                        .media_attachments
                        .into_iter()
                        .map(|media| store::operations::Medium {
                            url: media.url,
                            alt: media.description.unwrap_or_default(),
                        })
                        .collect(),
                    external: status.card.map_or_else(
                        || source::LiveExternal::None,
                        |card| {
                            source::LiveExternal::Some(store::operations::External {
                                uri: card.url,
                                title: card.title,
                                description: card.description,
                                thumb_url: card.image,
                            })
                        },
                    ),
                    created_at: status.created_at.into(),
                }
            })
            .collect();

        Ok(statuses)
    }

    #[allow(unused)]
    async fn post(
        &mut self,
        content: &str,
        facets: &[store::operations::Facet],
        reply_identifier: Option<&str>,
        images: Vec<store::operations::Medium>,
        external: Option<store::operations::External>,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        todo!();
    }

    #[allow(unused)]
    async fn repost(
        &mut self,
        identifier: &str,
        created_at: &DateTime<FixedOffset>,
    ) -> Result<String> {
        todo!();
    }

    #[allow(unused)]
    async fn delete(&mut self, identifier: &str) -> Result<()> {
        todo!();
    }
}
