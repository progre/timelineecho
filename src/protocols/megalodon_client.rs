use anyhow::Result;
use html2text::render::text_renderer::RichAnnotation;
use megalodon::{megalodon::GetAccountStatusesInputOptions, Megalodon};
use reqwest::header::HeaderMap;
use tracing::{event_enabled, trace, Level};

use crate::store;

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

fn link(current_idx: usize, uri: &str) -> store::Facet {
    store::Facet::Link {
        byte_slice: (current_idx as u32)..(current_idx as u32) + (uri.as_bytes().len() as u32),
        uri: uri.to_owned(),
    }
}

fn html_to_content_facets(html: &str) -> (String, Vec<store::Facet>) {
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
    megalodon: Box<dyn Megalodon>,
    account_id: Option<String>,
}

impl Client {
    pub fn new_mastodon(origin: String, access_token: String) -> Self {
        Self::new(megalodon::generator(
            megalodon::SNS::Mastodon,
            origin,
            Some(access_token),
            None,
        ))
    }

    fn new(megalodon: Box<dyn Megalodon>) -> Self {
        Self {
            megalodon,
            account_id: None,
        }
    }

    async fn account_id(&mut self) -> Result<&str> {
        if self.account_id.is_some() {
            return Ok(self.account_id.as_ref().unwrap());
        }
        let resp = self.megalodon.verify_account_credentials().await?;
        trace_header(&resp.header);
        self.account_id = Some(resp.json().id);
        Ok(self.account_id.as_ref().unwrap())
    }

    pub async fn fetch_statuses(&mut self) -> Result<(String, Vec<store::CreatingStatus>)> {
        let account_id = self.account_id().await?.to_owned();
        let resp = self
            .megalodon
            .get_account_statuses(
                account_id.clone(),
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
                store::CreatingStatus {
                    src_identifier: status.id,
                    content,
                    facets,
                    reply_src_identifier: status.in_reply_to_id,
                    media: status
                        .media_attachments
                        .into_iter()
                        .map(|media| store::Medium {
                            url: media.url,
                            alt: media.description.unwrap_or_default(),
                        })
                        .collect(),
                    external: status.card.map(|card| store::External {
                        uri: card.url,
                        title: card.title,
                        description: card.description,
                        thumb_url: card.image.unwrap_or_default(),
                    }),
                    created_at: status.created_at.to_rfc3339(),
                }
            })
            .rev()
            .rev()
            .collect();

        Ok((account_id, statuses))
    }
}
