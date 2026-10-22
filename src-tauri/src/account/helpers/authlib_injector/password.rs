use serde::{Deserialize, Serialize};
use serde_json::json;
use sjmcl_types::error::SJMCLResult;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;

use crate::account::helpers::authlib_injector::common::{
  RAW_RESPONSE_LOG_LIMIT, parse_profile, retrieve_profile, retrieve_profile_typed,
  truncate_raw_response,
};
use crate::account::helpers::authlib_injector::models::MinecraftProfile;
use crate::account::models::{AccountError, PlayerInfo};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct YggdrasilSession {
  #[serde(alias = "access_token")]
  access_token: Option<String>,
  #[serde(alias = "selected_profile")]
  selected_profile: Option<YggdrasilProfile>,
  #[serde(alias = "available_profiles")]
  available_profiles: Option<Vec<YggdrasilProfile>>,
}

// "id" and "name" have no case variants, so both camelCase and snake_case
// responses deserialize into this struct without extra aliases.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct YggdrasilProfile {
  id: String,
  name: String,
}

fn parse_session(raw: &str) -> Result<YggdrasilSession, AccountError> {
  serde_json::from_str(raw).map_err(|err| {
    log::warn!(
      "Failed to parse Yggdrasil response ({err}); first {RAW_RESPONSE_LOG_LIMIT} chars of the raw response: {}",
      truncate_raw_response(raw)
    );
    AccountError::ParseError
  })
}

/// Pick the profile a refresh response refers to: the explicitly selected one, else the first
/// available one, else the profile already known for this player.
fn resolve_refresh_profile(
  session: &YggdrasilSession,
  known_uuid: &str,
  known_name: &str,
) -> YggdrasilProfile {
  session
    .selected_profile
    .clone()
    .or_else(|| {
      session
        .available_profiles
        .as_ref()
        .and_then(|profiles| profiles.first().cloned())
    })
    .unwrap_or_else(|| YggdrasilProfile {
      id: known_uuid.to_string(),
      name: known_name.to_string(),
    })
}

/// Keep every profile that could be fetched, skipping the characters the auth server no longer
/// knows (a `204`/`404` session server response is reported as `NotFound`).
///
/// A login only fails when no character could be fetched at all; in that case the last failure is
/// reported, so a real error is not hidden behind the first unknown profile.
fn select_available_profiles(
  results: Vec<Result<MinecraftProfile, AccountError>>,
) -> Result<Vec<MinecraftProfile>, AccountError> {
  let mut profiles = vec![];
  let mut last_error = None;

  for result in results {
    match result {
      Ok(profile) => profiles.push(profile),
      Err(AccountError::NotFound) => {
        log::warn!("Skipping a character the auth server does not know (NotFound)");
      }
      Err(err) => {
        log::warn!("Failed to fetch a character profile: {err}");
        last_error = Some(err);
      }
    }
  }

  if profiles.is_empty() {
    Err(last_error.unwrap_or(AccountError::NotFound))
  } else {
    Ok(profiles)
  }
}

pub async fn login(
  app: &AppHandle,
  auth_server_url: String,
  username: String,
  password: String,
) -> SJMCLResult<(Vec<PlayerInfo>, bool)> {
  let client = app.state::<reqwest::Client>();

  let response = client
    .post(format!("{}/authserver/authenticate", auth_server_url))
    .json(&json!({
      "username": username,
      "password": password,
      "agent": {
        "name": "Minecraft",
        "version": 1
      },
    }))
    .send()
    .await
    .map_err(|_| AccountError::NetworkError)?;

  if !response.status().is_success() {
    return Err(AccountError::Invalid.into());
  }

  let raw_response = response
    .text()
    .await
    .map_err(|_| AccountError::ParseError)?;
  let content = parse_session(&raw_response)?;
  let access_token = content.access_token;
  let available_profiles = content.available_profiles.unwrap_or_default();

  if access_token.is_none() {
    log::warn!(
      "Yggdrasil authenticate response from {auth_server_url} has no access token; trying the profiles it contains instead"
    );
  }

  if let Some(selected_profile) = content.selected_profile {
    let id = selected_profile.id;
    let profile = retrieve_profile(app, auth_server_url.clone(), id).await?;

    Ok((
      vec![
        parse_profile(
          app,
          &profile,
          access_token,
          None,
          Some(auth_server_url),
          Some(username),
        )
        .await?,
      ],
      true,
    ))
  } else {
    if available_profiles.is_empty() {
      // An authenticate response without a token and without any profile carries nothing usable,
      // keep reporting it as a malformed response.
      return Err(
        if access_token.is_none() {
          AccountError::ParseError
        } else {
          AccountError::NotFound
        }
        .into(),
      );
    }

    let mut results = Vec::with_capacity(available_profiles.len());

    for profile in available_profiles {
      results.push(retrieve_profile_typed(app, auth_server_url.clone(), profile.id).await);
    }

    let profiles = select_available_profiles(results)?;
    let mut players = vec![];

    for mc_profile in profiles {
      let player = parse_profile(
        app,
        &mc_profile,
        access_token.clone(),
        None,
        Some(auth_server_url.clone()),
        Some(username.clone()),
      )
      .await?;

      players.push(player);
    }

    Ok((players, false))
  }
}

pub async fn refresh(
  app: &AppHandle,
  player: &PlayerInfo,
  is_new_bind: bool,
) -> SJMCLResult<PlayerInfo> {
  let client = app.state::<reqwest::Client>();
  let auth_server_url = player.auth_server_url.clone().unwrap_or_default();

  let response = client
    .post(format!("{auth_server_url}/authserver/refresh"))
    .json(&YggdrasilSession {
      access_token: Some(player.access_token.clone().unwrap_or_default()),
      selected_profile: if is_new_bind {
        Some(YggdrasilProfile {
          id: player.uuid.as_simple().to_string(),
          name: player.name.clone(),
        })
      } else {
        None
      },
      available_profiles: None,
    })
    .send()
    .await
    .map_err(|_| AccountError::NetworkError)?;

  if !response.status().is_success() {
    return Err(AccountError::Expired.into());
  }

  let raw_response = response
    .text()
    .await
    .map_err(|_| AccountError::ParseError)?;
  let content = parse_session(&raw_response)?;
  let access_token = content
    .access_token
    .clone()
    .or_else(|| player.access_token.clone());
  let known_uuid = player.uuid.as_simple().to_string();
  let selected_profile = resolve_refresh_profile(&content, &known_uuid, &player.name);

  let profile = retrieve_profile(app, auth_server_url, selected_profile.id).await?;

  parse_profile(
    app,
    &profile,
    access_token,
    None,
    player.auth_server_url.clone(),
    player.auth_account.clone(),
  )
  .await
}

#[cfg(test)]
mod tests {
  use std::sync::{Mutex, OnceLock};

  use super::{
    AccountError, YggdrasilProfile, YggdrasilSession, parse_session, resolve_refresh_profile,
    select_available_profiles,
  };
  use crate::account::helpers::authlib_injector::common::truncate_raw_response;
  use crate::account::helpers::authlib_injector::models::MinecraftProfile;

  const CAMEL_CASE_RESPONSE: &str = r#"{
    "accessToken": "camel-access-token",
    "clientToken": "camel-client-token",
    "selectedProfile": {
      "id": "069a79f444e94726a5befca90e38aaf5",
      "name": "Notch"
    },
    "availableProfiles": [
      { "id": "069a79f444e94726a5befca90e38aaf5", "name": "Notch" },
      { "id": "853c80ef3c3749fdaa49938b674adae6", "name": "jeb_" }
    ]
  }"#;

  const SNAKE_CASE_RESPONSE: &str = r#"{
    "access_token": "snake-access-token",
    "client_token": "snake-client-token",
    "selected_profile": {
      "id": "069a79f444e94726a5befca90e38aaf5",
      "name": "Notch"
    },
    "available_profiles": [
      { "id": "069a79f444e94726a5befca90e38aaf5", "name": "Notch" }
    ]
  }"#;

  const SNAKE_CASE_WITHOUT_SELECTED_PROFILE: &str = r#"{
    "access_token": "snake-access-token",
    "available_profiles": [
      { "id": "853c80ef3c3749fdaa49938b674adae6", "name": "jeb_" }
    ]
  }"#;

  #[test]
  fn parses_camel_case_authenticate_response() {
    let session = parse_session(CAMEL_CASE_RESPONSE).expect("camelCase response should parse");

    assert_eq!(session.access_token.as_deref(), Some("camel-access-token"));
    assert_eq!(
      session
        .selected_profile
        .as_ref()
        .map(|profile| profile.name.as_str()),
      Some("Notch")
    );
    assert_eq!(
      session
        .available_profiles
        .as_ref()
        .map(|profiles| profiles.len()),
      Some(2)
    );
  }

  #[test]
  fn parses_snake_case_authenticate_response() {
    let session = parse_session(SNAKE_CASE_RESPONSE).expect("snake_case response should parse");

    assert_eq!(session.access_token.as_deref(), Some("snake-access-token"));
    assert_eq!(
      session
        .selected_profile
        .as_ref()
        .map(|profile| profile.name.as_str()),
      Some("Notch")
    );
    assert_eq!(
      session
        .available_profiles
        .as_ref()
        .map(|profiles| profiles.len()),
      Some(1)
    );
  }

  #[test]
  fn parses_response_without_selected_profile_when_available_profiles_present() {
    let session = parse_session(SNAKE_CASE_WITHOUT_SELECTED_PROFILE)
      .expect("a response with available profiles should not be a parse error");

    assert!(session.selected_profile.is_none());
    assert_eq!(
      session
        .available_profiles
        .as_ref()
        .and_then(|profiles| profiles.first())
        .map(|profile| profile.name.as_str()),
      Some("jeb_")
    );
  }

  #[test]
  fn parses_response_without_access_token_when_profiles_present() {
    let session = parse_session(
      r#"{"selected_profile": {"id": "069a79f444e94726a5befca90e38aaf5", "name": "Notch"}}"#,
    )
    .expect("a profile-only response should not be a parse error");

    assert!(session.access_token.is_none());
    assert_eq!(
      session
        .selected_profile
        .as_ref()
        .map(|profile| profile.id.as_str()),
      Some("069a79f444e94726a5befca90e38aaf5")
    );
  }

  #[test]
  fn refresh_profile_falls_back_to_first_available_profile() {
    let session = parse_session(SNAKE_CASE_WITHOUT_SELECTED_PROFILE).unwrap();
    let profile = resolve_refresh_profile(&session, "known-uuid", "KnownPlayer");

    assert_eq!(profile.id, "853c80ef3c3749fdaa49938b674adae6");
    assert_eq!(profile.name, "jeb_");
  }

  #[test]
  fn refresh_profile_falls_back_to_known_player_uuid() {
    let session = parse_session(r#"{"access_token": "token"}"#).unwrap();
    let profile = resolve_refresh_profile(&session, "known-uuid", "KnownPlayer");

    assert_eq!(profile.id, "known-uuid");
    assert_eq!(profile.name, "KnownPlayer");
  }

  #[test]
  fn refresh_profile_prefers_selected_profile() {
    let session = parse_session(SNAKE_CASE_RESPONSE).unwrap();
    let profile = resolve_refresh_profile(&session, "known-uuid", "KnownPlayer");

    assert_eq!(profile.id, "069a79f444e94726a5befca90e38aaf5");
    assert_eq!(profile.name, "Notch");
  }

  #[test]
  fn invalid_json_returns_parse_error() {
    let err = parse_session("<html>not json</html>").unwrap_err();

    assert!(matches!(err, AccountError::ParseError));
  }

  #[test]
  fn raw_response_preview_is_truncated_to_200_chars() {
    let short = "short response";
    assert_eq!(truncate_raw_response(short), short);

    let long = "错".repeat(500);
    let preview = truncate_raw_response(&long);
    assert_eq!(preview.chars().count(), 200);
    assert!(preview.is_char_boundary(preview.len()));
  }

  fn profile(id: &str) -> MinecraftProfile {
    MinecraftProfile {
      id: id.to_string(),
      name: format!("player-{id}"),
      properties: None,
    }
  }

  #[test]
  fn multi_profile_login_skips_unknown_first_profile() {
    // The first character answers 204/404 (NotFound), the second one is fine: the login succeeds.
    let profiles =
      select_available_profiles(vec![Err(AccountError::NotFound), Ok(profile("second"))])
        .expect("one known profile is enough to finish the login");

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, "second");
  }

  #[test]
  fn multi_profile_login_keeps_all_known_profiles() {
    let profiles = select_available_profiles(vec![
      Ok(profile("first")),
      Err(AccountError::NotFound),
      Ok(profile("third")),
    ])
    .expect("known profiles should be kept");

    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].id, "first");
    assert_eq!(profiles[1].id, "third");
  }

  #[test]
  fn multi_profile_login_reports_last_error_when_every_profile_fails() {
    let error = select_available_profiles(vec![
      Err(AccountError::NotFound),
      Err(AccountError::ParseError),
      Err(AccountError::NetworkError),
    ])
    .err()
    .expect("no profile could be fetched");

    assert!(matches!(error, AccountError::NetworkError));
    assert_eq!(error.to_string(), "NETWORK_ERROR");
  }

  #[test]
  fn multi_profile_login_reports_not_found_when_every_profile_is_unknown() {
    let error = select_available_profiles(vec![
      Err(AccountError::NotFound),
      Err(AccountError::NotFound),
    ])
    .err()
    .expect("no profile could be fetched");

    assert!(matches!(error, AccountError::NotFound));
    assert_eq!(error.to_string(), "NOT_FOUND");
  }

  #[test]
  fn multi_profile_login_reports_not_found_for_an_empty_selection() {
    let error = select_available_profiles(vec![])
      .err()
      .expect("an empty selection cannot produce players");

    assert!(matches!(error, AccountError::NotFound));
    assert_eq!(error.to_string(), "NOT_FOUND");
  }

  static CAPTURED_LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());
  static LOGGER_READY: OnceLock<bool> = OnceLock::new();

  struct CapturingLogger;

  impl log::Log for CapturingLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
      metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
      if self.enabled(record.metadata())
        && let Ok(mut logs) = CAPTURED_LOGS.lock()
      {
        logs.push(record.args().to_string());
      }
    }

    fn flush(&self) {}
  }

  static LOGGER: CapturingLogger = CapturingLogger;

  fn capture_logs(action: impl FnOnce()) -> Option<String> {
    let ready = LOGGER_READY.get_or_init(|| {
      let installed = log::set_logger(&LOGGER).is_ok();
      if installed {
        log::set_max_level(log::LevelFilter::Warn);
      }
      installed
    });
    if !ready {
      return None;
    }

    CAPTURED_LOGS.lock().unwrap().clear();
    action();
    Some(CAPTURED_LOGS.lock().unwrap().join("\n"))
  }

  #[test]
  fn parse_failure_logs_raw_response_preview() {
    const MARKER: &str = "WTUMC_RAW_RESPONSE_MARKER";
    let raw = format!("<html>{MARKER}</html>");

    let logs = capture_logs(|| {
      let err = parse_session(&raw).unwrap_err();
      assert!(matches!(err, AccountError::ParseError));
    })
    .expect("test logger should be installable");

    assert!(
      logs.contains(MARKER),
      "raw response preview should be logged, got: {logs}"
    );
  }

  #[test]
  fn refresh_request_still_serializes_camel_case_keys() {
    let session = YggdrasilSession {
      access_token: Some("token".to_string()),
      selected_profile: Some(YggdrasilProfile {
        id: "known-uuid".to_string(),
        name: "KnownPlayer".to_string(),
      }),
      available_profiles: None,
    };

    let body = serde_json::to_value(&session).unwrap();
    assert_eq!(body["accessToken"], "token");
    assert_eq!(body["selectedProfile"]["id"], "known-uuid");
    assert!(body.get("selected_profile").is_none());
  }
}
