use html2text::render::text_renderer::RichAnnotation;

use crate::{sources::source, store};

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

impl From<megalodon::entities::Status> for source::LiveStatus {
    fn from(value: megalodon::entities::Status) -> Self {
        if let Some(reblog) = value.reblog {
            source::LiveStatus::Repost(store::operations::CreateRepostOperationStatus {
                src_identifier: value.id,
                target_src_identifier: reblog.id,
                target_src_uri: reblog.uri,
                created_at: value.created_at.into(),
            })
        } else {
            let (content, facets) = html_to_content_facets(&value.content);
            source::LiveStatus::Post(source::LivePost {
                identifier: value.id,
                uri: value.uri,
                content,
                facets,
                reply_src_identifier: value.in_reply_to_id,
                media: value
                    .media_attachments
                    .into_iter()
                    .map(|media| store::operations::Medium {
                        url: media.url,
                        alt: media.description.unwrap_or_default(),
                    })
                    .collect(),
                external: value.card.map_or_else(
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
                created_at: value.created_at.into(),
            })
        }
    }
}
