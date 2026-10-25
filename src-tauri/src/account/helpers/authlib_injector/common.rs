use serde_json::json;
use sjmcl_types::error::SJMCLResult;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest::{self, StatusCode};
use uuid::Uuid;

use crate::account::helpers::authlib_injector::models::MinecraftProfile;
use crate::account::helpers::authlib_injector::texture::AuthlibInjectorTextureOperation;
use crate::account::helpers::authlib_injector::{oauth, password};
use crate::account::helpers::texture::TextureOperation;
use crate::account::models::{AccountError, AuthServer, PlayerInfo, PlayerType};

/// Maximum number of characters of a raw response kept in diagnostic logs.
pub(crate) const RAW_RESPONSE_LOG_LIMIT: usize = 200;

/// Keep the beginning of a raw response so a parse failure can be diagnosed from the log.
pub(crate) fn truncate_raw_response(raw: &str) -> String {
  raw.chars().take(RAW_RESPONSE_LOG_LIMIT).collect()
}

/// Map the status code of a session server profile request to a typed error.
///
/// The session server answers `204 No Content` (and some servers `404 Not Found`) when it does
/// not know the requested profile; such a response carries no body, so it must never reach the
/// JSON parser and be reported as a parse error.
fn profile_status_error(status: StatusCode) -> Option<AccountError> {
  match status {
    StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => Some(AccountError::NotFound),
    status if status.is_success() => None,
    StatusCode::UNAUTHORIZED => Some(AccountError::Expired),
    StatusCode::FORBIDDEN => Some(AccountError::Forbidden),
    StatusCode::TOO_MANY_REQUESTS => Some(AccountError::TooManyRequests),
    status if status.is_server_error() => Some(AccountError::ServiceUnavailable),
    _ => Some(AccountError::NetworkError),
  }
}

fn parse_profile_response(raw: &str) -> Result<MinecraftProfile, AccountError> {
  serde_json::from_str::<MinecraftProfile>(raw).map_err(|err| {
    log::warn!(
      "Failed to parse Minecraft profile response ({err}); first {RAW_RESPONSE_LOG_LIMIT} chars of the raw response: {}",
      truncate_raw_response(raw)
    );
    AccountError::ParseError
  })
}

/// Fetch a profile from the session server, keeping the typed error so callers can tell an
/// unknown profile (`NotFound`) apart from a real failure.
pub(crate) async fn retrieve_profile_typed(
  app: &AppHandle,
  auth_server_url: String,
  id: String,
) -> Result<MinecraftProfile, AccountError> {
  let client = app.state::<reqwest::Client>();
  let response = client
    .get(format!(
      "{}/sessionserver/session/minecraft/profile/{}",
      auth_server_url, id
    ))
    .send()
    .await
    .map_err(|_| AccountError::NetworkError)?;

  if let Some(error) = profile_status_error(response.status()) {
    return Err(error);
  }

  let raw = response
    .text()
    .await
    .map_err(|_| AccountError::ParseError)?;

  parse_profile_response(&raw)
}

pub async fn retrieve_profile(
  app: &AppHandle,
  auth_server_url: String,
  id: String,
) -> SJMCLResult<MinecraftProfile> {
  Ok(retrieve_profile_typed(app, auth_server_url, id).await?)
}

pub async fn parse_profile(
  app: &AppHandle,
  profile: &MinecraftProfile,
  access_token: Option<String>,
  refresh_token: Option<String>,
  auth_server_url: Option<String>,
  auth_account: Option<String>,
) -> SJMCLResult<PlayerInfo> {
  let uuid = Uuid::parse_str(&profile.id).map_err(|_| AccountError::ParseError)?;
  let name = profile.name.clone();
  let textures = AuthlibInjectorTextureOperation::parse_skin(app, profile).await?;

  Ok(
    PlayerInfo {
      id: "".to_string(),
      uuid,
      name: name.to_string(),
      player_type: PlayerType::ThirdParty,
      auth_account,
      access_token,
      access_token_expires: None,
      refresh_token,
      textures,
      auth_server_url,
    }
    .with_generated_id(),
  )
}

pub async fn validate(app: &AppHandle, player: &PlayerInfo) -> SJMCLResult<bool> {
  let client = app.state::<reqwest::Client>();

  let response = client
    .post(format!(
      "{}/authserver/validate",
      player.auth_server_url.clone().unwrap_or_default()
    ))
    .json(&json!({
      "accessToken": player.access_token.clone()
    }))
    .send()
    .await
    .map_err(|_| AccountError::NetworkError)?;

  Ok(response.status().is_success())
}

pub async fn refresh(
  app: &AppHandle,
  player: &PlayerInfo,
  auth_server: &AuthServer,
) -> SJMCLResult<PlayerInfo> {
  if player.refresh_token.is_none() || Some("") == player.refresh_token.as_deref() {
    // to be compatible with legacy version of account config
    password::refresh(app, player, false).await
  } else {
    oauth::refresh(
      app,
      player,
      auth_server.client_id.clone(),
      auth_server.features.openid_configuration_url.clone(),
    )
    .await
  }
}

#[cfg(test)]
mod tests {
  use super::{RAW_RESPONSE_LOG_LIMIT, parse_profile_response, profile_status_error};
  use crate::account::models::AccountError;
  use tauri_plugin_http::reqwest::StatusCode;

  #[test]
  fn no_content_profile_response_is_not_found() {
    let error = profile_status_error(StatusCode::NO_CONTENT).expect("204 is not a success body");

    assert!(matches!(error, AccountError::NotFound));
    assert_eq!(error.to_string(), "NOT_FOUND");
  }

  #[test]
  fn not_found_profile_response_is_not_found() {
    let error = profile_status_error(StatusCode::NOT_FOUND).expect("404 is not a success body");

    assert!(matches!(error, AccountError::NotFound));
    assert_eq!(error.to_string(), "NOT_FOUND");
  }

  #[test]
  fn success_profile_response_has_no_status_error() {
    assert!(profile_status_error(StatusCode::OK).is_none());
  }

  #[test]
  fn forbidden_profile_response_is_forbidden() {
    let error = profile_status_error(StatusCode::FORBIDDEN).expect("403 is not a success body");

    assert!(matches!(error, AccountError::Forbidden));
    assert_eq!(error.to_string(), "FORBIDDEN");
  }

  #[test]
  fn server_error_profile_response_is_service_unavailable() {
    let error =
      profile_status_error(StatusCode::INTERNAL_SERVER_ERROR).expect("500 is not a success body");

    assert!(matches!(error, AccountError::ServiceUnavailable));
    assert_eq!(error.to_string(), "SERVICE_UNAVAILABLE");
  }

  #[test]
  fn unexpected_status_profile_response_is_network_error() {
    // A redirect/status nobody handled must not be reported as a malformed response body.
    let error = profile_status_error(StatusCode::BAD_REQUEST).expect("400 is not a success body");

    assert!(matches!(error, AccountError::NetworkError));
    assert_eq!(error.to_string(), "NETWORK_ERROR");
  }

  #[test]
  fn empty_success_body_is_parse_error() {
    let error = parse_profile_response("")
      .err()
      .expect("empty body cannot be a profile");

    assert!(matches!(error, AccountError::ParseError));
    assert_eq!(error.to_string(), "PARSE_ERROR");
  }

  #[test]
  fn invalid_json_profile_is_parse_error_without_panic() {
    let raw = format!("<html>{}</html>", "错误".repeat(300));
    let error = parse_profile_response(&raw)
      .err()
      .expect("html cannot be a profile");

    assert!(matches!(error, AccountError::ParseError));
    assert_eq!(error.to_string(), "PARSE_ERROR");
    assert!(raw.chars().count() > RAW_RESPONSE_LOG_LIMIT);
  }

  #[test]
  fn valid_profile_json_still_parses() {
    let profile = parse_profile_response(
      r#"{"id": "069a79f444e94726a5befca90e38aaf5", "name": "Notch", "properties": []}"#,
    )
    .expect("a valid profile response should parse");

    assert_eq!(profile.name, "Notch");
  }
}
