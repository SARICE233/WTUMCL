use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;

use crate::discover::models::{NewsPostResponse, NewsPostSummary, NewsSourceInfo};

pub const WTUMC_NEWS_ENDPOINT: &str = "https://wtumc.top:64053/api/news";
pub const WTUMC_HOME: &str = "https://wtumc.top:64053";
pub const WTUMC_ICON: &str = "https://wtumc.top:64053/icon-512.png";
const PAGE_SIZE: u64 = 12;
const ABSTRACT_MAX_CHARS: usize = 140;

#[derive(Debug, Deserialize, Default)]
struct WtumcEnvelope {
  #[serde(default)]
  data: WtumcNewsPage,
}

#[derive(Debug, Deserialize, Default)]
struct WtumcNewsPage {
  #[serde(default)]
  data: Vec<WtumcNewsItem>,
  #[serde(default)]
  total: u64,
}

#[derive(Debug, Deserialize, Default)]
struct WtumcNewsItem {
  #[serde(default)]
  id: u64,
  #[serde(default)]
  title_en: String,
  #[serde(default)]
  title_zh: String,
  #[serde(default)]
  content_en: String,
  #[serde(default)]
  content_zh: String,
  #[serde(default)]
  date: String,
  #[serde(default)]
  category: String,
  #[serde(default)]
  image: String,
}

pub fn wtumc_source_info(endpoint_url: String) -> NewsSourceInfo {
  NewsSourceInfo {
    name: "WTUMC".to_string(),
    full_name: "武汉纺织大学 MC 社区".to_string(),
    endpoint_url,
    icon_src: WTUMC_ICON.to_string(),
  }
}

fn pick(zh: &str, en: &str) -> String {
  let zh = zh.trim();
  if zh.is_empty() {
    en.trim().to_string()
  } else {
    zh.to_string()
  }
}

fn summarize(content: &str) -> String {
  let flat = content.split_whitespace().collect::<Vec<_>>().join(" ");
  if flat.chars().count() <= ABSTRACT_MAX_CHARS {
    flat
  } else {
    format!(
      "{}…",
      flat.chars().take(ABSTRACT_MAX_CHARS).collect::<String>()
    )
  }
}

impl From<(WtumcNewsItem, NewsSourceInfo)> for NewsPostSummary {
  fn from((item, source): (WtumcNewsItem, NewsSourceInfo)) -> Self {
    let image_src = if item.image.trim().is_empty() {
      (String::new(), 0, 0)
    } else {
      (item.image.trim().to_string(), 0, 0)
    };

    NewsPostSummary {
      title: pick(&item.title_zh, &item.title_en),
      abstracts: summarize(&pick(&item.content_zh, &item.content_en)),
      keywords: item.category,
      image_src,
      source,
      create_at: format!("{}T00:00:00Z", item.date.trim()),
      link: format!("{}/news/{}", WTUMC_HOME, item.id),
    }
  }
}

pub async fn fetch_wtumc_news_page(
  client: &ClientWithMiddleware,
  base_url: &str,
  cursor: Option<u64>,
) -> Option<(String, NewsPostResponse)> {
  let offset = cursor.unwrap_or(0);

  let response = client
    .get(WTUMC_NEWS_ENDPOINT)
    .query(&[
      ("limit", PAGE_SIZE.to_string()),
      ("offset", offset.to_string()),
    ])
    .send()
    .await
    .ok()?;

  if !response.status().is_success() {
    return None;
  }

  let envelope: WtumcEnvelope = response.json().await.ok()?;
  let page = envelope.data;
  let total = page.total;
  let source_info = wtumc_source_info(base_url.to_string());

  let posts: Vec<NewsPostSummary> = page
    .data
    .into_iter()
    .map(|item| NewsPostSummary::from((item, source_info.clone())))
    .collect();

  let next = offset + PAGE_SIZE < total;

  Some((
    base_url.to_string(),
    NewsPostResponse {
      posts,
      next: if next { Some(offset + PAGE_SIZE) } else { None },
      cursors: None,
    },
  ))
}
