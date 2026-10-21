use serde_json::Value;

use crate::account::models::AuthServerInfo;
use crate::utils::web::normalize_url;

pub static AUTHLIB_INJECTOR_JAR_NAME: &str = "authlib-injector.jar";

/// Static display information for a built-in auth server preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetAuthServer {
  pub auth_url: &'static str,
  pub name: &'static str,
  pub homepage_url: &'static str,
  pub register_url: &'static str,
}

pub static PRESET_AUTH_SERVERS: [PresetAuthServer; 3] = [
  PresetAuthServer {
    auth_url: "https://wtumc.top:64053/api/yggdrasil",
    name: "WTU-MC 社区皮肤站",
    homepage_url: "https://wtumc.top:64053",
    register_url: "https://wtumc.top:64053/register",
  },
  PresetAuthServer {
    auth_url: "https://skin.mualliance.ltd/api/yggdrasil",
    name: "MUA 皮肤站",
    homepage_url: "",
    register_url: "",
  },
  PresetAuthServer {
    auth_url: "https://littleskin.cn/api/yggdrasil",
    name: "LittleSkin",
    homepage_url: "",
    register_url: "",
  },
];

pub fn find_preset_auth_server(auth_url: &str) -> Option<&'static PresetAuthServer> {
  let normalized = normalize_url(auth_url);
  PRESET_AUTH_SERVERS
    .iter()
    .find(|preset| normalize_url(preset.auth_url) == normalized)
}

pub fn fill_missing_preset_auth_servers(auth_servers: &mut Vec<AuthServerInfo>) -> bool {
  let existing: std::collections::HashSet<String> = auth_servers
    .iter()
    .map(|info| normalize_url(&info.auth_url))
    .collect();

  let missing: Vec<AuthServerInfo> = PRESET_AUTH_SERVERS
    .iter()
    .filter(|preset| !existing.contains(&normalize_url(preset.auth_url)))
    .map(|preset| AuthServerInfo {
      auth_url: preset.auth_url.to_string(),
      client_id: None,
      metadata: Value::Null,
      timestamp: 0,
    })
    .collect();

  if missing.is_empty() {
    return false;
  }

  let mut merged = Vec::with_capacity(missing.len() + auth_servers.len());
  merged.extend(missing);
  merged.append(auth_servers);
  *auth_servers = merged;

  true
}

pub static SCOPE: &str =
  "openid offline_access Yggdrasil.PlayerProfiles.Select Yggdrasil.Server.Join";

pub static CLIENT_IDS: [(&str, &str); 6] = [
  // built-in preset auth servers
  ("skin.mualliance.ltd", "27"),
  ("littleskin.cn", "1014"),
  // supported MUA auth servers (ref: https://github.com/SJMC-Dev/SJMCL-client-ids)
  ("hall.mc.taru.xj.cn", "yggc_kPHIuWBoMhlkThX1"),
  ("oauth.ustb.world", "6"),
  ("skin.jsumc.fun", "2"),
  ("user.suesmc.ltd", "4"),
];

#[cfg(test)]
mod tests {
  use super::{PRESET_AUTH_SERVERS, fill_missing_preset_auth_servers, find_preset_auth_server};
  use crate::account::models::AuthServerInfo;
  use serde_json::{Value, json};

  fn server(auth_url: &str, metadata: Value) -> AuthServerInfo {
    AuthServerInfo {
      auth_url: auth_url.to_string(),
      client_id: None,
      metadata,
      timestamp: 0,
    }
  }

  fn urls(auth_servers: &[AuthServerInfo]) -> Vec<String> {
    auth_servers
      .iter()
      .map(|info| info.auth_url.clone())
      .collect()
  }

  #[test]
  fn fills_all_presets_into_empty_list() {
    let mut auth_servers = Vec::new();

    assert!(fill_missing_preset_auth_servers(&mut auth_servers));

    assert_eq!(auth_servers.len(), PRESET_AUTH_SERVERS.len());
    assert_eq!(
      urls(&auth_servers),
      vec![
        "https://wtumc.top:64053/api/yggdrasil",
        "https://skin.mualliance.ltd/api/yggdrasil",
        "https://littleskin.cn/api/yggdrasil",
      ]
    );
    assert!(auth_servers.iter().all(|info| info.metadata.is_null()));
  }

  #[test]
  fn fills_only_missing_presets_at_the_front() {
    let mut auth_servers = vec![
      server("https://littleskin.cn/api/yggdrasil", Value::Null),
      server("https://example.com/api/yggdrasil", json!({"meta": {}})),
    ];

    assert!(fill_missing_preset_auth_servers(&mut auth_servers));

    assert_eq!(
      urls(&auth_servers),
      vec![
        "https://wtumc.top:64053/api/yggdrasil",
        "https://skin.mualliance.ltd/api/yggdrasil",
        "https://littleskin.cn/api/yggdrasil",
        "https://example.com/api/yggdrasil",
      ]
    );
    assert_eq!(
      auth_servers[0].auth_url,
      "https://wtumc.top:64053/api/yggdrasil"
    );
  }

  #[test]
  fn complete_list_is_idempotent_and_keeps_order() {
    let mut auth_servers = vec![
      server("https://littleskin.cn/api/yggdrasil", Value::Null),
      server("https://wtumc.top:64053/api/yggdrasil", Value::Null),
      server("https://skin.mualliance.ltd/api/yggdrasil", Value::Null),
    ];
    let expected = urls(&auth_servers);

    assert!(!fill_missing_preset_auth_servers(&mut auth_servers));
    assert_eq!(urls(&auth_servers), expected);

    assert!(!fill_missing_preset_auth_servers(&mut auth_servers));
    assert_eq!(urls(&auth_servers), expected);
    assert_eq!(auth_servers.len(), PRESET_AUTH_SERVERS.len());
  }

  #[test]
  fn fill_does_not_overwrite_existing_metadata() {
    let metadata = json!({"meta": {"serverName": "Fetched name"}});
    let mut auth_servers = vec![server(
      "https://skin.mualliance.ltd/api/yggdrasil",
      metadata.clone(),
    )];

    assert!(fill_missing_preset_auth_servers(&mut auth_servers));

    let kept = auth_servers
      .iter()
      .find(|info| info.auth_url == "https://skin.mualliance.ltd/api/yggdrasil")
      .expect("existing server should stay in the list");
    assert_eq!(kept.metadata, metadata);
    assert_eq!(auth_servers.len(), PRESET_AUTH_SERVERS.len());
  }

  #[test]
  fn every_preset_has_a_static_display_name() {
    for preset in &PRESET_AUTH_SERVERS {
      assert!(
        !preset.name.is_empty(),
        "missing name for {}",
        preset.auth_url
      );
      assert!(!preset.auth_url.is_empty());
      assert!(preset.auth_url.starts_with("https://"));
    }
    assert_eq!(PRESET_AUTH_SERVERS[0].name, "WTU-MC 社区皮肤站");
  }

  #[test]
  fn deduplicates_by_normalized_url() {
    let mut auth_servers = vec![server(
      "HTTPS://WTUMC.TOP:64053/api/yggdrasil/",
      Value::Null,
    )];

    assert!(fill_missing_preset_auth_servers(&mut auth_servers));

    assert_eq!(
      urls(&auth_servers),
      vec![
        "https://skin.mualliance.ltd/api/yggdrasil",
        "https://littleskin.cn/api/yggdrasil",
        "HTTPS://WTUMC.TOP:64053/api/yggdrasil/",
      ]
    );
    assert!(find_preset_auth_server("HTTPS://WTUMC.TOP:64053/api/yggdrasil/").is_some());
    assert!(find_preset_auth_server("").is_none());
    assert!(find_preset_auth_server("not a url").is_none());
  }
}
