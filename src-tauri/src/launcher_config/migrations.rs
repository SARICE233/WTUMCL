use serde::Deserialize;
use serde::de::Deserializer;
use serde_json::Value;

use crate::launcher_config::models::{
  AppearanceBackgroundConfig, default_discover_source_endpoints,
};

// Migrate old built-in wallpaper choices to the new default preset.
const LEGACY_BUILT_IN_BACKGROUNDS: &[&str] =
  &["%built-in:Jokull", "%built-in:GNLXC", "%built-in:Florwyn"];

// Historical upstream default discover endpoint sets, all fully enabled.
// - Three entries: the post-JSUMC upstream default.
// - Two entries: the pre-JSUMC upstream default.
// - Two entries without the general article source: configurations written by
//   older launchers that shipped only these two community sources.
const LEGACY_DISCOVER_ENDPOINT_SETS: &[&[(&str, bool)]] = &[
  &[
    ("https://mc.sjtu.cn/api-sjmcl/article", true),
    ("https://mc.sjtu.cn/api-sjmcl/article/mua", true),
    ("https://api.jsumc.fun/sjmcl?type=tribee&src=535", true),
  ],
  &[
    ("https://mc.sjtu.cn/api-sjmcl/article", true),
    ("https://mc.sjtu.cn/api-sjmcl/article/mua", true),
  ],
  &[
    ("https://mc.sjtu.cn/api-sjmcl/article/mua", true),
    ("https://api.jsumc.fun/sjmcl?type=tribee&src=535", true),
  ],
];

fn is_legacy_discover_endpoint_set(endpoints: &[(String, bool)]) -> bool {
  LEGACY_DISCOVER_ENDPOINT_SETS.iter().any(|legacy| {
    legacy.len() == endpoints.len()
      && legacy.iter().all(|(url, enabled)| {
        endpoints
          .iter()
          .any(|(stored, stored_enabled)| stored.as_str() == *url && stored_enabled == enabled)
      })
  })
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct BackgroundPayload {
  choice: String,
  random_custom: bool,
  auto_darken: bool,
}

pub fn deserialize_background<'de, D>(
  deserializer: D,
) -> Result<AppearanceBackgroundConfig, D::Error>
where
  D: Deserializer<'de>,
{
  let mut payload = BackgroundPayload::deserialize(deserializer)?;

  if LEGACY_BUILT_IN_BACKGROUNDS.contains(&payload.choice.as_str()) {
    payload.choice = "%built-in:WTUMC".to_string();
    payload.auto_darken = false;
  }

  Ok(AppearanceBackgroundConfig {
    choice: payload.choice,
    random_custom: payload.random_custom,
    auto_darken: payload.auto_darken,
  })
}

// Deserializing discover sources from old and new formats.
// Migrated from Vec<String> to Vec<(String, bool)> with default enabled=true
pub fn deserialize_discover_sources<'de, D>(
  deserializer: D,
) -> Result<Vec<(String, bool)>, D::Error>
where
  D: Deserializer<'de>,
{
  let value = match Value::deserialize(deserializer) {
    Ok(value) => value,
    Err(_) => return Ok(Vec::default()),
  };

  let items = match value.as_array() {
    Some(items) => items,
    None => return Ok(Vec::default()),
  };

  let endpoints: Vec<(String, bool)> = items
    .iter()
    .filter_map(|item| match item {
      Value::String(url) => Some((url.to_string(), true)),
      Value::Array(tuple) if tuple.len() == 2 => {
        let url = tuple.first()?.as_str()?;
        let enabled = tuple.get(1)?.as_bool()?;
        Some((url.to_string(), enabled))
      }
      _ => None,
    })
    .collect();

  if is_legacy_discover_endpoint_set(&endpoints) {
    return Ok(default_discover_source_endpoints());
  }

  Ok(endpoints)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::launcher_config::models::LauncherConfig;

  fn load_background(value: Value) -> AppearanceBackgroundConfig {
    let config: LauncherConfig = serde_json::from_value(serde_json::json!({
      "appearance": { "background": value },
    }))
    .expect("launcher config with background should deserialize");

    config.appearance.background
  }

  #[test]
  fn background_field_default_is_the_wtumc_preset() {
    assert_eq!(
      LauncherConfig::default().appearance.background.choice,
      "%built-in:WTUMC"
    );
  }

  #[test]
  fn migrates_legacy_built_in_backgrounds_to_wtumc() {
    for legacy in ["%built-in:Jokull", "%built-in:GNLXC", "%built-in:Florwyn"] {
      let background = load_background(serde_json::json!({
        "choice": legacy,
        "randomCustom": true,
        "autoDarken": true,
      }));

      assert_eq!(background.choice, "%built-in:WTUMC");
      assert!(background.random_custom);
      assert!(!background.auto_darken);
    }
  }

  #[test]
  fn preserves_current_preset_and_custom_backgrounds() {
    for choice in ["%built-in:WTUMC", "my-background.jpg"] {
      let background = load_background(serde_json::json!({
        "choice": choice,
        "randomCustom": false,
        "autoDarken": true,
      }));

      assert_eq!(background.choice, choice);
      assert!(background.auto_darken);
    }
  }

  #[test]
  fn background_migration_is_idempotent() {
    let migrated = load_background(serde_json::json!({
      "choice": "%built-in:Florwyn",
    }));
    let migrated_again = load_background(serde_json::to_value(&migrated).unwrap());

    assert_eq!(migrated, migrated_again);
  }

  fn load_sources(value: Value) -> Vec<(String, bool)> {
    let config: LauncherConfig = serde_json::from_value(serde_json::json!({
      "discoverSourceEndpoints": value,
    }))
    .expect("launcher config with discover sources should deserialize");

    config.discover_source_endpoints
  }

  fn wtumc_default_sources() -> Vec<(String, bool)> {
    vec![("https://wtumc.top:64053/api/news".to_string(), true)]
  }

  #[test]
  fn field_default_is_the_wtumc_source() {
    assert_eq!(
      LauncherConfig::default().discover_source_endpoints,
      wtumc_default_sources()
    );
  }

  #[test]
  fn migrates_legacy_default_sets_ignoring_order() {
    let legacy_sets = [
      // Post-JSUMC three-entry upstream default, shuffled.
      serde_json::json!([
        ["https://api.jsumc.fun/sjmcl?type=tribee&src=535", true],
        ["https://mc.sjtu.cn/api-sjmcl/article/mua", true],
        ["https://mc.sjtu.cn/api-sjmcl/article", true],
      ]),
      // Pre-JSUMC two-entry upstream default.
      serde_json::json!([
        ["https://mc.sjtu.cn/api-sjmcl/article/mua", true],
        ["https://mc.sjtu.cn/api-sjmcl/article", true],
      ]),
      // Two-entry default written by older builds without the general source.
      serde_json::json!([
        ["https://api.jsumc.fun/sjmcl?type=tribee&src=535", true],
        ["https://mc.sjtu.cn/api-sjmcl/article/mua", true],
      ]),
    ];

    for legacy in legacy_sets {
      assert_eq!(load_sources(legacy), wtumc_default_sources());
    }
  }

  #[test]
  fn migrates_legacy_string_array_format() {
    let legacy_strings = serde_json::json!([
      "https://mc.sjtu.cn/api-sjmcl/article/mua",
      "https://mc.sjtu.cn/api-sjmcl/article",
      "https://api.jsumc.fun/sjmcl?type=tribee&src=535",
    ]);

    assert_eq!(load_sources(legacy_strings), wtumc_default_sources());
  }

  #[test]
  fn preserves_custom_and_partially_modified_sources() {
    let custom = serde_json::json!([
      ["https://example.com/news", true],
      ["https://mc.sjtu.cn/api-sjmcl/article", true],
    ]);
    assert_eq!(
      load_sources(custom),
      vec![
        ("https://example.com/news".to_string(), true),
        ("https://mc.sjtu.cn/api-sjmcl/article".to_string(), true),
      ]
    );

    let disabled_legacy_entry = serde_json::json!([
      ["https://mc.sjtu.cn/api-sjmcl/article", false],
      ["https://mc.sjtu.cn/api-sjmcl/article/mua", true],
      ["https://api.jsumc.fun/sjmcl?type=tribee&src=535", true],
    ]);
    assert_eq!(
      load_sources(disabled_legacy_entry),
      vec![
        ("https://mc.sjtu.cn/api-sjmcl/article".to_string(), false),
        ("https://mc.sjtu.cn/api-sjmcl/article/mua".to_string(), true),
        (
          "https://api.jsumc.fun/sjmcl?type=tribee&src=535".to_string(),
          true
        ),
      ]
    );

    let removed_source = serde_json::json!([
      ["https://mc.sjtu.cn/api-sjmcl/article", true],
      ["https://api.jsumc.fun/sjmcl?type=tribee&src=535", true],
    ]);
    assert_eq!(
      load_sources(removed_source),
      vec![
        ("https://mc.sjtu.cn/api-sjmcl/article".to_string(), true),
        (
          "https://api.jsumc.fun/sjmcl?type=tribee&src=535".to_string(),
          true
        ),
      ]
    );

    let added_source = serde_json::json!([
      ["https://mc.sjtu.cn/api-sjmcl/article", true],
      ["https://mc.sjtu.cn/api-sjmcl/article/mua", true],
      ["https://api.jsumc.fun/sjmcl?type=tribee&src=535", true],
      ["https://example.com/news", true],
    ]);
    assert_eq!(
      load_sources(added_source),
      vec![
        ("https://mc.sjtu.cn/api-sjmcl/article".to_string(), true),
        ("https://mc.sjtu.cn/api-sjmcl/article/mua".to_string(), true),
        (
          "https://api.jsumc.fun/sjmcl?type=tribee&src=535".to_string(),
          true
        ),
        ("https://example.com/news".to_string(), true),
      ]
    );
  }

  #[test]
  fn preserves_custom_legacy_string_format() {
    let legacy_strings = serde_json::json!(["https://example.com/news"]);

    assert_eq!(
      load_sources(legacy_strings),
      vec![("https://example.com/news".to_string(), true)]
    );
  }

  #[test]
  fn legacy_config_without_custom_mirror_url_loads_with_empty_default() {
    let legacy: LauncherConfig = serde_json::from_value(serde_json::json!({
      "download": { "source": { "strategy": "mirror" } },
    }))
    .expect("legacy launcher config without customMirrorUrl should deserialize");

    assert_eq!(legacy.download.source.strategy, "mirror");
    assert_eq!(legacy.download.source.custom_mirror_url, "");
    assert!(
      LauncherConfig::default()
        .download
        .source
        .custom_mirror_url
        .is_empty()
    );

    // The new field is serialized with the camelCase key the frontend writes.
    let serialized = serde_json::to_value(LauncherConfig::default()).unwrap();
    assert_eq!(
      serialized["download"]["source"]["customMirrorUrl"],
      serde_json::json!("")
    );
  }

  #[test]
  fn migration_is_idempotent() {
    let legacy = serde_json::json!([
      ["https://mc.sjtu.cn/api-sjmcl/article", true],
      ["https://mc.sjtu.cn/api-sjmcl/article/mua", true],
      ["https://api.jsumc.fun/sjmcl?type=tribee&src=535", true],
    ]);

    let migrated = load_sources(legacy);
    let migrated_again = load_sources(serde_json::to_value(&migrated).unwrap());

    assert_eq!(migrated, wtumc_default_sources());
    assert_eq!(migrated_again, migrated);
  }

  #[test]
  fn empty_and_invalid_values_do_not_panic() {
    assert_eq!(load_sources(serde_json::json!([])), Vec::new());
    assert_eq!(load_sources(Value::Null), Vec::new());
    assert_eq!(
      load_sources(serde_json::json!({ "url": "https://example.com/news" })),
      Vec::new()
    );
    assert_eq!(
      load_sources(serde_json::json!([
        42,
        ["https://example.com/only-url"],
        ["https://example.com/news", "yes"],
        { "url": "https://example.com/news" }
      ])),
      Vec::new()
    );
  }
}

#[cfg(test)]
mod login_flag_migration_tests {
  use crate::launcher_config::models::LauncherConfig;

  #[test]
  fn legacy_config_without_split_login_flags_still_loads() {
    // A pre-split config only knew allow_full_login_feature; the new flags must
    // fall back to "allowed" so no user loses a login option on upgrade.
    let legacy = r#"{"basicInfo":{"allowFullLoginFeature":false}}"#;
    let config: LauncherConfig = serde_json::from_str(legacy).expect("legacy config should load");

    assert!(config.basic_info.allow_third_party_login);
    assert!(config.basic_info.allow_offline_login);
    assert!(!config.basic_info.allow_full_login_feature);
  }
}
