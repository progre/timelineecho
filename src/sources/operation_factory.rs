use anyhow::Result;
use futures::future::join_all;
use tracing::warn;

use crate::store::{
    self,
    operations::{DeleteRepostOperationStatus, Facet::Link},
    user::SourceStatus,
};

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
    facets: &[store::operations::Facet],
    http_client: &reqwest::Client,
) -> Result<Option<store::operations::External>> {
    for facet in facets {
        match facet {
            Link { byte_slice: _, uri } => {
                let html = match fetch_html(http_client, uri.clone()).await {
                    Ok(ok) => ok,
                    Err(err) => {
                        warn!("extract external from facet failed: {}", err);
                        continue;
                    }
                };
                return Ok(Some(store::operations::External {
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

async fn try_into_operation(live: LiveStatus, http_client: &reqwest::Client) -> Result<Operation> {
    Ok(match live {
        LiveStatus::Post(post) => {
            let external = match post.external {
                LiveExternal::Some(external) => Some(external),
                LiveExternal::None => None,
                LiveExternal::Unknown => create_external(&post.facets, http_client).await?,
            };
            Operation::CreatePost(store::operations::CreatePostOperationStatus {
                src_identifier: post.identifier,
                src_uri: post.uri,
                content: post.content,
                facets: post.facets,
                reply_src_identifier: post.reply_src_identifier,
                media: post.media,
                external,
                created_at: post.created_at,
            })
        }
        LiveStatus::Repost(repost) => Operation::CreateRepost(repost),
    })
}

pub async fn create_operations(
    http_client: &reqwest::Client,
    live_statuses: &[LiveStatus],
    stored_statuses: &[store::user::SourceStatus],
) -> Result<Vec<Operation>> {
    if live_statuses.is_empty() || stored_statuses.is_empty() {
        return Ok(vec![]);
    }
    // C
    let last_date_time = stored_statuses
        .iter()
        .max_by_key(|status| status.created_at())
        .map(SourceStatus::created_at);
    let c = live_statuses
        .iter()
        .filter(|live| {
            last_date_time.map_or(true, |last_date_time| live.created_at() > last_date_time)
        })
        .filter(|live| {
            if let LiveStatus::Post(post) = live {
                // 自分宛てのリプライのみを投稿対象にする
                if let Some(reply_src_identifier) = &post.reply_src_identifier {
                    return live_statuses.iter().any(|live| match live {
                        LiveStatus::Post(post) => &post.identifier == reply_src_identifier,
                        LiveStatus::Repost(_) => false,
                    });
                }
            }
            true
        })
        .map(|live| try_into_operation(live.clone(), http_client));
    let c = join_all(c).await.into_iter().collect::<Result<Vec<_>>>()?;
    // UD
    let since = &live_statuses
        .iter()
        .min_by_key(|status| status.created_at())
        .unwrap()
        .created_at();
    let ud = stored_statuses
        .iter()
        .filter(|stored| stored.created_at() >= since)
        .filter_map(|stored| match stored {
            store::user::SourceStatus::Post(post) => {
                let live = live_statuses
                    .iter()
                    .filter_map(|live| match live {
                        LiveStatus::Post(live) => Some(live),
                        LiveStatus::Repost(_) => None,
                    })
                    .find(|live| live.identifier == post.identifier);
                if let Some(live) = live {
                    if live.content == post.content {
                        return None;
                    }
                    Some(Operation::UpdatePost(
                        store::operations::UpdatePostOperationStatus {
                            src_identifier: live.identifier.clone(),
                            content: live.content.clone(),
                            facets: live.facets.clone(),
                        },
                    ))
                } else {
                    Some(Operation::DeletePost(
                        store::operations::DeletePostOperationStatus {
                            src_identifier: post.identifier.clone(),
                        },
                    ))
                }
            }
            store::user::SourceStatus::Repost(repost) => {
                let live = live_statuses
                    .iter()
                    .filter_map(|live| match live {
                        LiveStatus::Post(_) => None,
                        LiveStatus::Repost(repost) => Some(repost),
                    })
                    .find(|live| live.src_identifier == repost.identifier);
                if live.is_some() {
                    None
                } else {
                    Some(Operation::DeleteRepost(DeleteRepostOperationStatus {
                        src_identifier: repost.identifier.clone(),
                    }))
                }
            }
        });

    Ok(c.into_iter().chain(ud).collect())
}
