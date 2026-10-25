use serde::{Deserialize, Serialize};
use serde_json::Value;
use sjmcl_types::storage::Storage;
use std::path::PathBuf;
use strum_macros::{Display, EnumIter, EnumString};
use uuid::Uuid;

use crate::APP_DATA_DIR;
use crate::account::constants::ACCOUNTS_FILE_NAME;
use crate::account::helpers::authlib_injector::constants::{
  PRESET_AUTH_SERVERS, find_preset_auth_server,
};
use crate::account::helpers::texture::draw_avatar;
use crate::utils::image::ImageWrapper;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum PlayerType {
  #[serde(rename = "offline")]
  Offline,
  #[serde(rename = "3rdparty")]
  ThirdParty,
  #[serde(rename = "microsoft")]
  Microsoft,
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Display, Default, EnumIter)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PresetRole {
  #[default]
  Steve,
  Alex,
}

#[derive(
  Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Display, Default, EnumIter, EnumString,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum TextureType {
  #[default]
  Skin,
  Cape,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Display, Default, EnumIter, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum SkinModel {
  #[default]
  Default,
  Slim,
}

impl<'de> Deserialize<'de> for SkinModel {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    match s.to_lowercase().as_str() {
      "default" | "classic" => Ok(SkinModel::Default),
      "slim" => Ok(SkinModel::Slim),
      _ => Err(serde::de::Error::unknown_variant(
        &s,
        &["default", "classic", "slim"],
      )),
    }
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
  pub texture_type: TextureType,
  pub image: ImageWrapper,
  pub model: SkinModel,
  pub preset: Option<PresetRole>,
}

/// A cape of a player, as listed by the cape commands.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCapeInfo {
  pub id: String,
  pub alias: String,
  pub image: ImageWrapper,
  pub active: bool,
}

// only for the client
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
  pub id: String,
  pub name: String,
  pub uuid: Uuid,
  pub avatar: Vec<ImageWrapper>, // [face, hat]
  pub player_type: PlayerType,
  pub auth_account: Option<String>,
  pub auth_server: Option<AuthServer>,
  pub access_token: Option<String>,
  pub access_token_expires: Option<chrono::DateTime<chrono::Utc>>,
  pub refresh_token: Option<String>,
  pub textures: Vec<Texture>,
}

impl Player {
  pub fn from_player_info(
    player_info: PlayerInfo,
    auth_servers: Option<&[AuthServerInfo]>,
  ) -> Self {
    let owned_auth_servers;
    let auth_servers = match auth_servers {
      Some(list) => list,
      None => {
        let state: AccountInfo = Storage::load().unwrap_or_default();
        owned_auth_servers = state.auth_servers.into_iter().collect::<Vec<_>>();
        &owned_auth_servers
      }
    };

    let auth_server = player_info.auth_server_url.clone().map(|auth_server_url| {
      AuthServer::from(
        auth_servers
          .iter()
          .find(|server| server.auth_url == auth_server_url)
          .cloned()
          .unwrap_or_default(),
      )
    });

    Player {
      id: player_info.id,
      name: player_info.name,
      uuid: player_info.uuid,
      avatar: draw_avatar(36, &player_info.textures[0].image.image),
      player_type: player_info.player_type,
      auth_account: player_info.auth_account,
      access_token: player_info.access_token,
      access_token_expires: player_info.access_token_expires,
      refresh_token: player_info.refresh_token,
      auth_server,
      textures: player_info.textures,
    }
  }
}

impl From<PlayerInfo> for Player {
  fn from(player_info: PlayerInfo) -> Self {
    Player::from_player_info(player_info, None)
  }
}

// for backend storage, without saving the whole auth server info
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
  pub id: String,
  pub name: String,
  pub uuid: Uuid,
  pub player_type: PlayerType,
  pub auth_account: Option<String>,
  pub auth_server_url: Option<String>,
  pub access_token: Option<String>,
  pub access_token_expires: Option<chrono::DateTime<chrono::Utc>>,
  pub refresh_token: Option<String>,
  pub textures: Vec<Texture>,
}

impl PlayerInfo {
  /// Generate ID from existing fields and return updated struct
  pub fn with_generated_id(mut self) -> Self {
    let server_identity = match self.player_type {
      PlayerType::Offline => "OFFLINE".to_string(),
      PlayerType::Microsoft => "MICROSOFT".to_string(),
      _ => self.auth_server_url.clone().unwrap_or_default(),
    };
    self.id = format!("{}:{}:{}", self.name, server_identity, self.uuid);
    self
  }
}

impl From<Player> for PlayerInfo {
  fn from(player: Player) -> Self {
    PlayerInfo {
      id: player.id,
      name: player.name,
      uuid: player.uuid,
      player_type: player.player_type,
      auth_account: player.auth_account,
      textures: player.textures,
      access_token: player.access_token,
      access_token_expires: player.access_token_expires,
      refresh_token: player.refresh_token,
      auth_server_url: player
        .auth_server
        .as_ref()
        .map(|server| server.auth_url.clone()),
    }
  }
}

impl PartialEq for PlayerInfo {
  fn eq(&self, another: &PlayerInfo) -> bool {
    self.name == another.name && self.auth_server_url == another.auth_server_url
  }
}

impl Eq for PlayerInfo {}

#[derive(Deserialize)]
// received from auth server, do not need camel case
pub struct DeviceAuthResponse {
  pub device_code: String,
  pub user_code: String,
  pub verification_uri: String,
  pub verification_uri_complete: Option<String>,
  pub interval: Option<u64>,
  pub expires_in: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
// communicate with the client
pub struct DeviceAuthResponseInfo {
  pub device_code: String,
  pub user_code: String,
  pub verification_uri: String,
  pub interval: Option<u64>,
  pub expires_in: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct OAuthTokens {
  pub access_token: String,
  pub refresh_token: String,
  pub id_token: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct OAuthErrorResponse {
  pub error: String,
  pub error_description: Option<String>,
  pub error_uri: Option<String>,
}

structstruck::strike! {
  #[strikethrough[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize, Default)]]
  #[strikethrough[serde(rename_all = "camelCase", deny_unknown_fields)]]
  pub struct AuthServer {
    pub name: String,
    pub auth_url: String,
    pub homepage_url: String,
    pub register_url: String,
    pub features: struct {
      pub non_email_login: bool,
      pub openid_configuration_url: String,
    },
    pub client_id: Option<String>,
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthServerInfo {
  pub auth_url: String,
  pub client_id: Option<String>,
  pub metadata: Value,
  pub timestamp: u64,
}

impl From<AuthServerInfo> for AuthServer {
  fn from(info: AuthServerInfo) -> Self {
    let preset = find_preset_auth_server(&info.auth_url);

    // Prefer the metadata returned by the server, but fall back to the static
    // display information of a built-in preset when the metadata is missing
    // (for example before the first successful refresh, or while offline).
    let display = |value: Option<&str>, preset_value: Option<&'static str>| -> String {
      match value {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => preset_value.unwrap_or_default().to_string(),
      }
    };

    AuthServer {
      name: display(
        info.metadata["meta"]["serverName"].as_str(),
        preset.map(|preset| preset.name),
      ),
      auth_url: info.auth_url.clone(),
      homepage_url: display(
        info.metadata["meta"]["links"]["homepage"].as_str(),
        preset.map(|preset| preset.homepage_url),
      ),
      register_url: display(
        info.metadata["meta"]["links"]["register"].as_str(),
        preset.map(|preset| preset.register_url),
      ),
      features: Features {
        non_email_login: info.metadata["meta"]["feature.non_email_login"]
          .as_bool()
          .unwrap_or(false),
        openid_configuration_url: info.metadata["meta"]["feature.openid_configuration_url"]
          .as_str()
          .unwrap_or_default()
          .to_string(),
      },
      client_id: info.client_id,
    }
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountInfo {
  pub players: Vec<PlayerInfo>,
  pub auth_servers: Vec<AuthServerInfo>,
  pub is_oauth_processing: bool,
}

impl Default for AccountInfo {
  fn default() -> Self {
    AccountInfo {
      players: vec![],
      auth_servers: PRESET_AUTH_SERVERS
        .iter()
        .map(|preset| AuthServerInfo {
          auth_url: preset.auth_url.to_string(),
          client_id: None,
          metadata: Value::Null,
          timestamp: 0,
        })
        .collect(),
      is_oauth_processing: false,
    }
  }
}

impl AccountInfo {
  pub fn get_player_by_id(&self, id: String) -> Option<&PlayerInfo> {
    self.players.iter().find(|player| player.id == id)
  }

  pub fn get_player_by_id_mut(&mut self, id: String) -> Option<&mut PlayerInfo> {
    self.players.iter_mut().find(|player| player.id == id)
  }
}

impl Storage for AccountInfo {
  fn file_path() -> PathBuf {
    APP_DATA_DIR.get().unwrap().join(ACCOUNTS_FILE_NAME)
  }
}

#[derive(Debug, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountError {
  Duplicate,
  Expired,
  FullLoginUnavailable,
  Invalid,
  NotFound,
  TextureError,
  NetworkError,
  ServiceUnavailable,
  TooManyRequests,
  Forbidden,
  UnknownProfile,
  CannotAddSelf,
  DuplicatedProfiles,
  ParseError,
  Cancelled,
  NoDownloadApi,
  NoTextureApi,
  SaveError,
  NoMinecraftProfile,
  XstsBanned,
  XstsParentalRestriction,
  XstsNoXboxAccount,
  XstsTermsNotAccepted,
  XstsRegionBanned,
  XstsChildAccount,
  XstsUnknownError,
}

impl std::error::Error for AccountError {}

#[cfg(test)]
mod tests {
  use super::{AccountInfo, AuthServer, AuthServerInfo};
  use crate::account::helpers::authlib_injector::constants::PRESET_AUTH_SERVERS;
  use serde_json::{Value, json};

  fn info(auth_url: &str, metadata: Value) -> AuthServerInfo {
    AuthServerInfo {
      auth_url: auth_url.to_string(),
      client_id: None,
      metadata,
      timestamp: 0,
    }
  }

  #[test]
  fn preset_auth_server_falls_back_to_static_info_when_metadata_is_null() {
    let wtumc_url = PRESET_AUTH_SERVERS[0].auth_url;
    let server = AuthServer::from(info(wtumc_url, Value::Null));

    assert_eq!(server.name, "WTU-MC 社区皮肤站");
    assert!(!server.name.is_empty());
    assert_eq!(server.homepage_url, "https://wtumc.top:64053");
    assert_eq!(server.register_url, "https://wtumc.top:64053/register");
  }

  #[test]
  fn preset_auth_server_falls_back_per_field_when_metadata_is_incomplete() {
    let wtumc_url = PRESET_AUTH_SERVERS[0].auth_url;
    let server = AuthServer::from(info(
      wtumc_url,
      json!({"meta": {"serverName": "Partial name"}}),
    ));

    assert_eq!(server.name, "Partial name");
    assert_eq!(server.homepage_url, "https://wtumc.top:64053");
    assert_eq!(server.register_url, "https://wtumc.top:64053/register");
  }

  #[test]
  fn preset_auth_server_prefers_fetched_metadata() {
    let wtumc_url = PRESET_AUTH_SERVERS[0].auth_url;
    let server = AuthServer::from(info(
      wtumc_url,
      json!({
        "meta": {
          "serverName": "Fetched name",
          "links": {
            "homepage": "https://fetched.example.com",
            "register": "https://fetched.example.com/register"
          }
        }
      }),
    ));

    assert_eq!(server.name, "Fetched name");
    assert_eq!(server.homepage_url, "https://fetched.example.com");
    assert_eq!(server.register_url, "https://fetched.example.com/register");
  }

  #[test]
  fn default_account_info_contains_all_presets() {
    let account = AccountInfo::default();

    assert_eq!(account.auth_servers.len(), PRESET_AUTH_SERVERS.len());
    assert_eq!(
      account.auth_servers[0].auth_url,
      "https://wtumc.top:64053/api/yggdrasil"
    );
    assert_eq!(
      AuthServer::from(account.auth_servers[0].clone()).name,
      "WTU-MC 社区皮肤站"
    );
  }

  #[test]
  fn non_preset_auth_server_keeps_empty_display_info() {
    let server = AuthServer::from(info("https://example.com/api/yggdrasil", Value::Null));

    assert!(server.name.is_empty());
    assert!(server.homepage_url.is_empty());
    assert!(server.register_url.is_empty());
  }
}
