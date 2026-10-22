use futures::StreamExt;
use futures::stream;
use sjmcl_types::error::SJMCLResult;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;

use crate::discover::helpers::hitokoto::{Hitokoto, fetch_hitokoto as fetch_hitokoto_with_client};
use crate::discover::helpers::mc_news::{MC_NEWS_ENDPOINT, fetch_mc_news_page};
use crate::discover::helpers::wtumc_news::{
  WTUMC_NEWS_ENDPOINT, fetch_wtumc_news_page, wtumc_source_info,
};
use crate::discover::models::{NewsPostRequest, NewsPostResponse, NewsSourceInfo};
use crate::launcher_config::models::LauncherConfig;
use crate::utils::web::with_retry;

/// Per-source deadline for the discovery feeds.
///
/// The shared client keeps its global retry budget (see `utils::web`), so the
/// only way to stop one source from holding the whole batch is to bound each
/// source at the call site. Eight seconds keeps a healthy source usable while an
/// unreachable one no longer blocks the rest of the news list.
const NEWS_SOURCE_TIMEOUT: Duration = Duration::from_secs(8);

/// Maximum number of source requests kept in flight at the same time.
const NEWS_SOURCE_CONCURRENCY: usize = 6;

/// The outcome of fetching one news source.
///
/// A failed source is represented by `Err` here instead of aborting the whole
/// batch, so the caller can return the results of the healthy sources and log
/// the failed ones.
struct SourceOutcome<T> {
  index: usize,
  endpoint_url: String,
  result: Result<T, String>,
}

/// Fetches every source with an independent timeout and bounded concurrency.
///
/// `buffer_unordered` yields results as soon as each source completes (instead
/// of waiting for the slowest one, like `join_all` did) and never runs more than
/// `concurrency` fetches at once. A source that fails or times out is reported
/// as `Err` in the returned vector; it does not cancel the other sources.
async fn fetch_sources_isolated<T, E, F>(
  sources: Vec<(String, F)>,
  per_source_timeout: Duration,
  concurrency: usize,
) -> Vec<SourceOutcome<T>>
where
  F: Future<Output = Result<T, E>>,
  E: std::fmt::Display,
{
  let tasks = sources
    .into_iter()
    .enumerate()
    .map(|(index, (endpoint_url, source))| async move {
      let result = match tokio::time::timeout(per_source_timeout, source).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!("timed out after {per_source_timeout:?}")),
      };
      if let Err(reason) = &result {
        log::warn!("News source {endpoint_url} failed: {reason}");
      }
      SourceOutcome {
        index,
        endpoint_url,
        result,
      }
    });

  stream::iter(tasks)
    .buffer_unordered(concurrency.max(1))
    .collect()
    .await
}

#[tauri::command]
pub async fn fetch_news_sources_info(app: AppHandle) -> SJMCLResult<Vec<NewsSourceInfo>> {
  let post_sources = {
    let binding = app.state::<Mutex<LauncherConfig>>();
    let state = binding.lock().unwrap();
    state.discover_source_endpoints.clone()
  };

  let client = with_retry(app.state::<reqwest::Client>().inner().clone());

  let tasks: Vec<_> = post_sources
    .into_iter()
    .map(|(url, _)| {
      let client = client.clone();
      let endpoint_url = url.clone();
      let task = async move {
        if url.starts_with(WTUMC_NEWS_ENDPOINT) {
          return Ok::<_, String>(wtumc_source_info(url));
        }

        let mut news_source = NewsSourceInfo {
          name: "".to_string(),
          full_name: "".to_string(),
          endpoint_url: url.clone(),
          icon_src: "".to_string(),
        };

        let response = client
          .get(&url)
          .query(&[("pageSize", "0")]) // ?pageSize=0
          .send()
          .await;

        if let Ok(response) = response {
          let json_data: serde_json::Value = response.json().await.unwrap_or_default();

          if let Some(source_info) = json_data.get("sourceInfo") {
            news_source.name = source_info["name"].as_str().unwrap_or("").to_string();
            news_source.full_name = source_info["fullName"].as_str().unwrap_or("").to_string();
            news_source.icon_src = source_info["iconSrc"].as_str().unwrap_or("").to_string();
          }
        }

        Ok(news_source)
      };
      (endpoint_url, task)
    })
    .collect();

  let mut outcomes =
    fetch_sources_isolated(tasks, NEWS_SOURCE_TIMEOUT, NEWS_SOURCE_CONCURRENCY).await;
  // `buffer_unordered` reports completion order; the UI expects the configured
  // order, so restore it before flattening the per-source results.
  outcomes.sort_by_key(|outcome| outcome.index);

  Ok(
    outcomes
      .into_iter()
      .map(|outcome| match outcome.result {
        Ok(info) => info,
        // Keep one entry per configured source, like the previous error path,
        // so the settings page still lists a source whose info timed out.
        Err(_) => NewsSourceInfo {
          endpoint_url: outcome.endpoint_url,
          ..Default::default()
        },
      })
      .collect(),
  )
}

#[tauri::command]
pub async fn fetch_news_post_summaries(
  app: AppHandle,
  requests: Vec<NewsPostRequest>,
) -> SJMCLResult<NewsPostResponse> {
  let client = with_retry(app.state::<reqwest::Client>().inner().clone());
  let tasks: Vec<_> = requests
    .into_iter()
    .map(|NewsPostRequest { url, cursor }| {
      let client = client.clone();
      let endpoint_url = url.clone();
      let task = async move {
        if url.starts_with(MC_NEWS_ENDPOINT) {
          return fetch_mc_news_page(&client, &url, cursor)
            .await
            .ok_or_else(|| "Minecraft news request returned no data".to_string());
        }

        if url.starts_with(WTUMC_NEWS_ENDPOINT) {
          return fetch_wtumc_news_page(&client, &url, cursor)
            .await
            .ok_or_else(|| "WTUMC news request returned no data".to_string());
        }

        let mut req = client.get(&url).query(&[("pageSize", "12")]);

        if let Some(c) = cursor {
          req = req.query(&[("cursor", &c.to_string())]);
        }

        let resp = req
          .send()
          .await
          .map_err(|error| format!("request failed: {error}"))?;
        if !resp.status().is_success() {
          return Err(format!("HTTP {}", resp.status()));
        }
        let mut parsed: NewsPostResponse = resp
          .json()
          .await
          .map_err(|error| format!("invalid response: {error}"))?;
        for post in &mut parsed.posts {
          post.source.endpoint_url = url.clone();
        }
        Ok((url, parsed))
      };
      (endpoint_url, task)
    })
    .collect();

  let results = fetch_sources_isolated(tasks, NEWS_SOURCE_TIMEOUT, NEWS_SOURCE_CONCURRENCY).await;

  let mut all_posts = Vec::new();
  let mut cursors_map = HashMap::new();

  for outcome in results {
    if let Ok((url, post_response)) = outcome.result {
      all_posts.extend(post_response.posts);
      if let Some(next_cursor) = post_response.next {
        cursors_map.insert(url, next_cursor);
      }
    }
    // The per-source failure is already logged by `fetch_sources_isolated`;
    // keep the healthy sources instead of failing the whole command.
  }

  all_posts.sort_by(|a, b| b.create_at.cmp(&a.create_at));

  Ok(NewsPostResponse {
    posts: all_posts,
    next: None,
    cursors: Some(cursors_map),
  })
}

#[tauri::command]
pub async fn fetch_hitokoto(app: AppHandle) -> SJMCLResult<Hitokoto> {
  let client = app.state::<reqwest::Client>().inner().clone();

  Ok(fetch_hitokoto_with_client(&client).await)
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::future::BoxFuture;
  use std::sync::Arc;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::time::Instant;

  #[tokio::test]
  async fn news_aggregation_returns_healthy_sources_when_another_times_out() {
    let tasks: Vec<(
      String,
      BoxFuture<'static, Result<&'static str, &'static str>>,
    )> = vec![
      (
        "slow".to_string(),
        Box::pin(async { futures::future::pending::<Result<&'static str, &'static str>>().await }),
      ),
      (
        "broken".to_string(),
        Box::pin(async { Err::<&'static str, &'static str>("connection refused") }),
      ),
      (
        "healthy".to_string(),
        Box::pin(async { Ok::<&'static str, &'static str>("post") }),
      ),
    ];

    let started = Instant::now();
    let outcomes = fetch_sources_isolated(tasks, Duration::from_millis(100), 3).await;
    let elapsed = started.elapsed();

    // A hanging source must not extend the batch beyond its own timeout.
    assert!(
      elapsed < Duration::from_secs(2),
      "aggregation took {elapsed:?}, expected the batch to return near the 100 ms source timeout"
    );
    assert_eq!(outcomes.len(), 3);

    let healthy: Vec<_> = outcomes
      .iter()
      .filter(|outcome| outcome.result.is_ok())
      .collect();
    assert_eq!(healthy.len(), 1);
    assert_eq!(healthy[0].endpoint_url, "healthy");
    assert_eq!(healthy[0].result.as_ref().unwrap(), &"post");

    // Every failed source carries its own status instead of cancelling the batch.
    for outcome in &outcomes {
      if outcome.result.is_err() {
        assert!(
          !outcome.result.as_ref().unwrap_err().is_empty(),
          "failed sources must carry a reason"
        );
      }
    }
    let slow = outcomes
      .iter()
      .find(|outcome| outcome.endpoint_url == "slow")
      .unwrap();
    assert!(slow.result.as_ref().unwrap_err().contains("timed out"));
    let broken = outcomes
      .iter()
      .find(|outcome| outcome.endpoint_url == "broken")
      .unwrap();
    assert_eq!(broken.result.as_ref().unwrap_err(), "connection refused");
  }

  #[tokio::test]
  async fn news_aggregation_never_exceeds_the_concurrency_limit() {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    let tasks: Vec<_> = (0..12)
      .map(|index| {
        let in_flight = in_flight.clone();
        let peak = peak.clone();
        (format!("source-{index}"), async move {
          let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
          peak.fetch_max(current, Ordering::SeqCst);
          tokio::time::sleep(Duration::from_millis(10)).await;
          in_flight.fetch_sub(1, Ordering::SeqCst);
          Ok::<_, String>(index)
        })
      })
      .collect();

    let outcomes = fetch_sources_isolated(tasks, Duration::from_secs(5), 4).await;

    assert_eq!(outcomes.len(), 12);
    assert!(
      outcomes.iter().all(|outcome| outcome.result.is_ok()),
      "no source should fail in this test"
    );
    assert!(
      peak.load(Ordering::SeqCst) <= 4,
      "at most four sources may be fetched at once, saw {}",
      peak.load(Ordering::SeqCst)
    );
    assert_eq!(in_flight.load(Ordering::SeqCst), 0);
  }

  #[tokio::test]
  async fn news_aggregation_returns_empty_for_an_empty_source_list() {
    let sources: Vec<(String, BoxFuture<'static, Result<&'static str, String>>)> = Vec::new();
    let outcomes = fetch_sources_isolated(sources, Duration::from_millis(10), 4).await;
    assert!(outcomes.is_empty());
  }
}
