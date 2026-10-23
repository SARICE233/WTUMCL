use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use sjmcl_types::storage::load_json_async;
use std::collections::HashMap;
use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;

use crate::instance::helpers::client_json::AssetIndexInfo;
use crate::instance::models::misc::InstanceError;
use crate::utils::fs::validate_sha1;

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct AssetIndex {
  pub objects: HashMap<String, AssetIndexItem>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct AssetIndexItem {
  pub hash: String,
  pub size: i64,
}

/// Whether the cached index really is the file the version manifest expects.
///
/// Existence used to be the only test, so a stale, truncated or foreign index
/// stayed in place forever. A wrong index does not fail loudly: it hides every
/// asset object it should have pointed at, which surfaces as an empty resource
/// pack and a black title screen.
fn cached_index_matches(path: &Path, expected: &AssetIndexInfo) -> bool {
  let Ok(metadata) = std::fs::metadata(path) else {
    return false;
  };
  if expected.size > 0 && metadata.len() != expected.size as u64 {
    return false;
  }
  validate_sha1(path.to_path_buf(), expected.sha1.clone()).is_ok()
}

/// The same check for bytes that have not been written yet: a mirror can answer
/// 200 with the wrong document, and caching that would make the bad index
/// permanent.
fn downloaded_index_matches(bytes: &[u8], expected: &AssetIndexInfo) -> bool {
  if expected.size > 0 && bytes.len() as i64 != expected.size {
    return false;
  }
  if expected.sha1.trim().is_empty() {
    return true;
  }
  let mut hasher = Sha1::new();
  hasher.update(bytes);
  hex::encode(hasher.finalize()) == expected.sha1
}

pub async fn load_asset_index(
  app: &AppHandle,
  asset_index_path: &Path,
  asset_index_info: &AssetIndexInfo,
) -> SJMCLResult<AssetIndex> {
  if cached_index_matches(asset_index_path, asset_index_info) {
    let asset_index = load_json_async::<AssetIndex>(asset_index_path)
      .await
      .map_err(|_| InstanceError::AssetIndexParseError)?;

    return Ok(asset_index);
  }

  let client = app.state::<reqwest::Client>().clone();

  let bytes = client
    .get(&asset_index_info.url)
    .send()
    .await
    .map_err(|_| InstanceError::NetworkError)?
    .error_for_status()
    .map_err(|_| InstanceError::NetworkError)?
    .bytes()
    .await
    .map_err(|_| InstanceError::NetworkError)?;

  if !downloaded_index_matches(&bytes, asset_index_info) {
    log::warn!(
      "Asset index {} from {} does not match the expected sha1/size; refusing to cache it",
      asset_index_info.id,
      asset_index_info.url
    );
    return Err(InstanceError::AssetIndexParseError.into());
  }

  // Cached verbatim: re-serialising the parsed document would change its bytes
  // and make the sha1 comparison fail on every later launch.
  if let Some(parent) = asset_index_path.parent() {
    tokio::fs::create_dir_all(parent)
      .await
      .map_err(|e| SJMCLError(e.to_string()))?;
  }
  tokio::fs::write(asset_index_path, &bytes)
    .await
    .map_err(|e| SJMCLError(e.to_string()))?;

  let asset_index = serde_json::from_slice::<AssetIndex>(&bytes)
    .map_err(|_| InstanceError::AssetIndexParseError)?;
  Ok(asset_index)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  fn unique_path(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    std::env::temp_dir().join(format!(
      "sjmcl-asset-index-{}-{}-{}",
      tag,
      std::process::id(),
      nanos
    ))
  }

  fn expected_for(bytes: &[u8]) -> AssetIndexInfo {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    AssetIndexInfo {
      id: "29".to_string(),
      sha1: hex::encode(hasher.finalize()),
      size: bytes.len() as i64,
      total_size: 0,
      url: "https://example.invalid/29.json".to_string(),
    }
  }

  #[test]
  fn a_cached_index_is_accepted_only_when_size_and_sha1_match() {
    const INDEX: &[u8] = br#"{"objects":{"a":{"hash":"aa","size":1}}}"#;
    let expected = expected_for(INDEX);
    let path = unique_path("cached");
    assert!(!cached_index_matches(&path, &expected));

    std::fs::write(&path, INDEX).unwrap();
    assert!(cached_index_matches(&path, &expected));

    // Truncated: same prefix, shorter file.
    std::fs::write(&path, &INDEX[..INDEX.len() - 1]).unwrap();
    assert!(!cached_index_matches(&path, &expected));

    // Same length, different bytes: the stale-index case that used to be
    // accepted forever because only existence was checked.
    let mut same_size = INDEX.to_vec();
    same_size[0] = b' ';
    std::fs::write(&path, &same_size).unwrap();
    assert!(!cached_index_matches(&path, &expected));

    // A pretty-printed copy is not the upstream file either.
    std::fs::write(
      &path,
      br#"{ "objects": { "a": { "hash": "aa", "size": 1 } } }"#,
    )
    .unwrap();
    assert!(!cached_index_matches(&path, &expected));

    let _ = std::fs::remove_file(&path);
  }

  #[test]
  fn a_downloaded_index_is_rejected_on_mismatch_and_a_sha1_free_manifest_falls_back_to_size() {
    const INDEX: &[u8] = br#"{"objects":{}}"#;
    let expected = expected_for(INDEX);
    assert!(downloaded_index_matches(INDEX, &expected));
    // One byte longer, so the announced size rejects it.
    assert!(!downloaded_index_matches(br#"{"objects": {}}"#, &expected));
    assert!(!downloaded_index_matches(br#"{"objectz":{}}"#, &expected));

    let without_sha1 = AssetIndexInfo {
      sha1: String::new(),
      ..expected_for(INDEX)
    };
    assert!(downloaded_index_matches(INDEX, &without_sha1));
    assert!(!downloaded_index_matches(b"{}", &without_sha1));
  }
}
