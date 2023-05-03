use anyhow::Result;
use futures::future::join_all;
use tracing::warn;

use crate::store;

use super::source::{LiveExternal, LiveStatus, Operation};

async fn fetch_html(http_client: &reqwest::Client, uri: String) -> Result<webpage::HTML> {
    let text = http_client
        .get(&uri)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(webpage::HTML::from_string(text, Some(uri))?)
}

async fn create_external(
    facets: &[store::Facet],
    http_client: &reqwest::Client,
) -> Result<Option<store::External>> {
    for facet in facets {
        match facet {
            store::Facet::Link { byte_slice: _, uri } => {
                let html = match fetch_html(http_client, uri.clone()).await {
                    Ok(ok) => ok,
                    Err(err) => {
                        warn!("extract external from facet failed: {}", err);
                        continue;
                    }
                };
                return Ok(Some(store::External {
                    uri: uri.clone(),
                    title: html.title.unwrap_or_default(),
                    description: html.description.unwrap_or_default(),
                    thumb_url: html.opengraph.images.first().map(|g| g.url.clone()),
                }));
            }
        }
    }
    Ok(None)
}

async fn try_into_source_status(
    live: LiveStatus,
    http_client: &reqwest::Client,
) -> Result<store::SourceStatusFull> {
    let external = match live.external {
        LiveExternal::Some(external) => Some(external),
        LiveExternal::None => None,
        LiveExternal::Unknown => create_external(&live.facets, http_client).await?,
    };
    Ok(store::SourceStatusFull {
        src_identifier: live.identifier,
        content: live.content,
        facets: live.facets,
        reply_src_identifier: live.reply_src_identifier,
        media: live.media,
        external,
        created_at: live.created_at,
    })
}

pub async fn create_operations(
    http_client: &reqwest::Client,
    live_statuses: &[LiveStatus],
    stored_statuses: &[store::SourceStatus],
) -> Result<Vec<Operation>> {
    if live_statuses.is_empty() || stored_statuses.is_empty() {
        return Ok(vec![]);
    }
    // C
    let last_id = stored_statuses
        .iter()
        .max_by_key(|status| &status.identifier)
        .map(|x| &x.identifier);
    let c = live_statuses
        .iter()
        .filter(|live| last_id.map_or(true, |last_id| &live.identifier > last_id))
        .map(|status| async {
            Ok(Operation::Create(
                try_into_source_status(status.clone(), http_client).await?,
            ))
        });
    let c = join_all(c).await.into_iter().collect::<Result<Vec<_>>>()?;
    // UD
    let since_id = &live_statuses
        .iter()
        .min_by_key(|status| &status.identifier)
        .unwrap()
        .identifier;
    let ud = stored_statuses
        .iter()
        .filter(|stored| &stored.identifier >= since_id)
        .filter_map(|stored| {
            let Some(live) = live_statuses.iter().find(|live| live.identifier == stored.identifier) else {
                return Some(Operation::Delete {
                    src_identifier: stored.identifier.clone(),
                });
            };
            if live.content != stored.content {
                return Some(Operation::Update {
                    src_identifier: live.identifier.clone(),
                    content: live.content.clone(),
                    facets: live.facets.clone(),
                });
            }
            None
        });

    Ok(c.into_iter().chain(ud).collect())
}
