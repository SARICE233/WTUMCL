use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tauri_plugin_http::reqwest;

const HITOKOTO_ENDPOINT: &str = "https://v1.hitokoto.cn/?c=a&c=d&c=i&c=k&encode=json";
const HITOKOTO_TIMEOUT_SECS: u64 = 5;

/// Sentences served when the remote endpoint is unreachable, slow or malformed.
const FALLBACK_HITOKOTO: [(&str, &str); 3] = [
  ("先把火把插在脚下，再去看远处的矿洞。", "WTUMC"),
  ("挖到基岩之前，别忘了先带一张床。", "WTUMC"),
  ("存档会记住你今天走过的每一步。", "WTUMC"),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hitokoto {
  pub text: String,
  pub from: String,
  pub from_who: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HitokotoResponse {
  hitokoto: String,
  from: Option<String>,
  from_who: Option<String>,
}

fn fallback() -> Hitokoto {
  static NEXT_INDEX: AtomicUsize = AtomicUsize::new(0);
  let index = NEXT_INDEX.fetch_add(1, Ordering::Relaxed) % FALLBACK_HITOKOTO.len();
  let (text, from) = FALLBACK_HITOKOTO[index];
  Hitokoto {
    text: text.to_string(),
    from: from.to_string(),
    from_who: None,
  }
}

fn parse_response(raw: &str) -> Option<Hitokoto> {
  let parsed = serde_json::from_str::<HitokotoResponse>(raw).ok()?;
  if parsed.hitokoto.trim().is_empty() {
    return None;
  }
  Some(Hitokoto {
    text: parsed.hitokoto,
    from: parsed.from.unwrap_or_default(),
    from_who: parsed.from_who,
  })
}

/// Never fails: every network/parse problem degrades to a local fallback sentence.
pub async fn fetch_hitokoto(client: &reqwest::Client) -> Hitokoto {
  let response = match client
    .get(HITOKOTO_ENDPOINT)
    .timeout(Duration::from_secs(HITOKOTO_TIMEOUT_SECS))
    .send()
    .await
  {
    Ok(response) => response,
    Err(_) => return fallback(),
  };

  if !response.status().is_success() {
    return fallback();
  }

  match response.text().await {
    Ok(raw) => parse_response(&raw).unwrap_or_else(fallback),
    Err(_) => fallback(),
  }
}

#[cfg(test)]
mod tests {
  use super::{fallback, parse_response};

  #[test]
  fn parses_well_formed_payload() {
    let hitokoto = parse_response(r#"{"hitokoto":"你好","from":"测试","from_who":"某人"}"#)
      .expect("payload should parse");
    assert_eq!(hitokoto.text, "你好");
    assert_eq!(hitokoto.from, "测试");
    assert_eq!(hitokoto.from_who.as_deref(), Some("某人"));
  }

  #[test]
  fn tolerates_missing_optional_fields() {
    let hitokoto = parse_response(r#"{"hitokoto":"你好"}"#).expect("payload should parse");
    assert_eq!(hitokoto.from, "");
    assert_eq!(hitokoto.from_who, None);
  }

  #[test]
  fn rejects_malformed_or_empty_payloads() {
    assert!(parse_response("not json").is_none());
    assert!(parse_response(r#"{"from":"测试"}"#).is_none());
    assert!(parse_response(r#"{"hitokoto":"   "}"#).is_none());
  }

  #[test]
  fn fallback_rotates_and_is_never_empty() {
    let first = fallback();
    let second = fallback();
    assert!(!first.text.is_empty());
    assert!(!second.text.is_empty());
    assert_ne!(first.text, second.text);
  }
}
