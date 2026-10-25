use sjmcl_types::error::SJMCLResult;
use sjmcl_types::storage::Storage;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest::{self, RequestBuilder, StatusCode};

use crate::account::constants::DEFAULT_POLLING_INTERVAL;
use crate::account::models::{
  AccountError, AccountInfo, DeviceAuthResponseInfo, OAuthErrorResponse, OAuthTokens, PlayerInfo,
  PlayerType,
};
use crate::launcher_config::models::LauncherConfig;
use crate::utils::image::{ImageWrapper, decode_image};
use crate::utils::web::is_china_mainland_ip;

pub async fn fetch_image(app: &AppHandle, url: String) -> SJMCLResult<ImageWrapper> {
  let client = app.state::<reqwest::Client>();

  let response = client
    .get(url.clone())
    .send()
    .await
    .map_err(|_| AccountError::NetworkError)?;

  // A failing texture host used to reach the image decoder, so its 403 or 404 body was
  // reported as a malformed image. Check the status first, and keep an expired token apart
  // from a transport problem so the client can show the real reason.
  let status = response.status();
  if !status.is_success() {
    log::warn!("Failed to download the texture image from {url}: HTTP {status}");
    return Err(image_status_error(status).into());
  }

  let img_bytes = response
    .bytes()
    .await
    .map_err(|_| AccountError::ParseError)?
    .to_vec();

  Ok(
    decode_image(img_bytes)
      .map_err(|_| AccountError::ParseError)?
      .into(),
  )
}

/// Map the status of a failed texture download to the account error the client can explain.
///
/// Only an unauthorized response means the access token went stale. Every other failure (a 403
/// of the texture host, a 404 of a removed texture, a 5xx of the CDN) is a transport problem:
/// reporting them as a parse error hides the real reason from the user.
pub fn image_status_error(status: StatusCode) -> AccountError {
  if status == StatusCode::UNAUTHORIZED {
    AccountError::Expired
  } else {
    AccountError::NetworkError
  }
}

pub fn get_selected_player_info(app: &AppHandle) -> SJMCLResult<PlayerInfo> {
  let account_binding = app.state::<Mutex<AccountInfo>>();
  let account_state = account_binding.lock()?;

  let config_binding = app.state::<Mutex<LauncherConfig>>();
  let config_state = config_binding.lock()?;

  let selected_player_id = &config_state.states.shared.selected_player_id;
  if selected_player_id.is_empty() {
    return Err(AccountError::NotFound.into());
  }

  let player_info = account_state
    .players
    .iter()
    .find(|player| player.id == *selected_player_id)
    .ok_or(AccountError::NotFound)?;

  Ok(player_info.clone())
}

pub fn add_player(app: &AppHandle, new_player: PlayerInfo) -> SJMCLResult<()> {
  let new_player_id = new_player.id.clone();
  {
    let account_binding = app.state::<Mutex<AccountInfo>>();
    let mut account_state = account_binding.lock()?;

    let config_binding = app.state::<Mutex<LauncherConfig>>();
    let mut config_state = config_binding.lock()?;

    if !can_add_player(&new_player.player_type, &config_state) {
      return Err(AccountError::FullLoginUnavailable.into());
    }

    if account_state
      .players
      .iter()
      .any(|player| player.id == new_player_id)
    {
      return Err(AccountError::Duplicate.into());
    }

    config_state.partial_update(
      app,
      "states.shared.selected_player_id",
      &serde_json::to_string(&new_player_id)?,
    )?;
    config_state.save()?;

    account_state.players.push(new_player);
    account_state.save()?;
  }

  Ok(())
}

pub fn get_player_by_id(app: &AppHandle, player_id: &str) -> SJMCLResult<Option<PlayerInfo>> {
  let account_binding = app.state::<Mutex<AccountInfo>>();
  let account_state = account_binding.lock()?;

  Ok(
    account_state
      .players
      .iter()
      .find(|player| player.id == player_id)
      .cloned(),
  )
}

pub fn update_player_by_id(app: &AppHandle, player_id: &str, info: PlayerInfo) -> SJMCLResult<()> {
  let account_binding = app.state::<Mutex<AccountInfo>>();
  let mut account_state = account_binding.lock()?;

  if let Some(index) = account_state
    .players
    .iter()
    .position(|player| player.id == player_id)
  {
    account_state.players[index] = info;
    account_state.save()?;
  }
  Ok(())
}

pub fn apply_login_availability(
  config: &mut LauncherConfig,
  allow_offline: bool,
) -> SJMCLResult<()> {
  config.basic_info.allow_third_party_login = true;
  config.basic_info.allow_offline_login = allow_offline;
  config.basic_info.allow_full_login_feature = allow_offline;
  Ok(())
}

/// Offline login is still gated by the microsoft feature set, third-party login
/// (authlib-injector servers) and microsoft login never are.
pub fn can_add_player(player_type: &PlayerType, config: &LauncherConfig) -> bool {
  match player_type {
    PlayerType::Offline => config.basic_info.allow_offline_login,
    _ => true,
  }
}

pub async fn check_full_login_availability(app: &AppHandle) -> SJMCLResult<()> {
  let loc_flag = is_china_mainland_ip(app).await;

  let account_binding = app.state::<Mutex<AccountInfo>>();
  let account_state = account_binding.lock()?;

  let config_binding = app.state::<Mutex<LauncherConfig>>();
  let mut config_state = config_binding.lock()?;

  match loc_flag {
    Some(true) => {
      // in China (mainland), offline and 3rd-party login are always available
      apply_login_availability(&mut config_state, true)?;
    }
    _ => {
      // not in China (mainland) or cannot determine the IP
      // offline login needs a microsoft account for skin/online services, so it
      // follows "has any player" (not only microsoft type, users may delete it);
      // third-party login never depends on the microsoft feature set.
      apply_login_availability(&mut config_state, !account_state.players.is_empty())?;
    }
  }

  config_state.save()?;
  Ok(())
}

pub async fn oauth_polling(
  app: &AppHandle,
  sender: RequestBuilder,
  auth_info: DeviceAuthResponseInfo,
) -> SJMCLResult<OAuthTokens> {
  let account_binding = app.state::<Mutex<AccountInfo>>();
  {
    let mut account_state = account_binding.lock()?;
    account_state.is_oauth_processing = true;
  }
  let mut interval = auth_info.interval.unwrap_or(DEFAULT_POLLING_INTERVAL);
  let start_time = std::time::Instant::now();
  loop {
    {
      let account_state = account_binding.lock()?;
      if !account_state.is_oauth_processing {
        return Err(AccountError::Cancelled.into());
      }
    }

    let response = sender
      .try_clone()
      .ok_or(AccountError::NetworkError)?
      .send()
      .await
      .map_err(|_| AccountError::NetworkError)?;

    if response.status().is_success() {
      return Ok(
        response
          .json()
          .await
          .map_err(|_| AccountError::ParseError)?,
      );
    } else {
      if response.status().as_u16() != 400 {
        return Err(AccountError::NetworkError.into());
      }

      let error_response: OAuthErrorResponse = response
        .json()
        .await
        .map_err(|_| AccountError::ParseError)?;

      match error_response.error.as_str() {
        "authorization_pending" => {
          // continue polling
        }
        "slow_down" => {
          interval += 5;
        }
        "access_denied" => {
          return Err(AccountError::Cancelled.into());
        }
        "expired_token" => {
          return Err(AccountError::Expired.into());
        }
        _ => {
          return Err(AccountError::NetworkError.into());
        }
      }
    }

    if start_time.elapsed().as_secs() >= auth_info.expires_in {
      return Err(AccountError::Expired.into());
    }

    tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
  }
}

#[cfg(test)]
mod login_flag_tests {
  use super::can_add_player;
  use crate::account::models::PlayerType;
  use crate::launcher_config::models::LauncherConfig;

  fn config(offline: bool) -> LauncherConfig {
    let mut config = LauncherConfig::default();
    config.basic_info.allow_offline_login = offline;
    config
  }

  #[test]
  fn third_party_login_ignores_the_microsoft_feature_flag() {
    let mut blocked = config(false);
    blocked.basic_info.allow_full_login_feature = false;
    assert!(can_add_player(&PlayerType::ThirdParty, &blocked));
    assert!(can_add_player(&PlayerType::Microsoft, &blocked));
  }

  #[test]
  fn offline_login_follows_its_own_flag() {
    assert!(!can_add_player(&PlayerType::Offline, &config(false)));
    assert!(can_add_player(&PlayerType::Offline, &config(true)));
  }
}

#[cfg(test)]
mod texture_download_tests {
  use super::image_status_error;
  use crate::account::models::AccountError;
  use crate::utils::image::decode_image;
  use tauri_plugin_http::reqwest::StatusCode;

  #[test]
  fn unauthorized_texture_download_is_reported_as_expired() {
    let error = image_status_error(StatusCode::UNAUTHORIZED);

    assert!(matches!(error, AccountError::Expired));
    assert_eq!(error.to_string(), "EXPIRED");
  }

  #[test]
  fn forbidden_texture_download_is_not_an_expired_token() {
    // A 403 from the texture host must not ask the user to log in again.
    let error = image_status_error(StatusCode::FORBIDDEN);

    assert!(matches!(error, AccountError::NetworkError));
    assert_eq!(error.to_string(), "NETWORK_ERROR");
  }

  #[test]
  fn missing_and_broken_texture_responses_are_network_errors() {
    for status in [
      StatusCode::NOT_FOUND,
      StatusCode::INTERNAL_SERVER_ERROR,
      StatusCode::BAD_GATEWAY,
    ] {
      assert_eq!(image_status_error(status).to_string(), "NETWORK_ERROR");
    }
  }

  #[test]
  fn an_html_error_page_is_not_decoded_as_a_texture() {
    // The body of a failed download (an HTML error page) must never be treated as image data.
    let html = b"<!DOCTYPE html><html><body>403 Forbidden</body></html>".to_vec();

    assert!(decode_image(html).is_err());
  }
}
