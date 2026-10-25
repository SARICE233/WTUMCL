use serde_json::json;
use sjmcl_types::error::SJMCLResult;
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;
use tauri_plugin_http::reqwest::multipart::{Form, Part};

use crate::account::helpers::microsoft::constants::ACTIVE_CAPE_ENDPOINT;
use crate::account::helpers::microsoft::models::MinecraftProfile;
use crate::account::helpers::microsoft::oauth::fetch_minecraft_profile;
use crate::account::helpers::misc::fetch_image;
use crate::account::helpers::texture::{TextureOperation, load_preset_skin_info};
use crate::account::models::{
  AccountError, PlayerCapeInfo, PlayerInfo, PresetRole, SkinModel, Texture, TextureType,
};
use crate::utils::image::{ImageWrapper, load_image_from_dir};

pub struct MicrosoftTextureOperation;

impl TextureOperation<MinecraftProfile> for MicrosoftTextureOperation {
  async fn parse_skin(app: &AppHandle, profile: &MinecraftProfile) -> SJMCLResult<Vec<Texture>> {
    let mut downloads = Vec::new();
    for pending in parse_texture_entries(profile) {
      let image = fetch_image(app, pending.url.clone()).await;
      downloads.push((pending, image));
    }
    let mut textures = assemble_textures(downloads);

    if !textures
      .iter()
      .any(|texture| texture.texture_type == TextureType::Skin)
    {
      // this player has no usable skin, use preset Steve skin instead: the avatar is drawn from
      // the first texture, so a skin has to stay first even when only a cape could be downloaded
      let (skin_path, model) = load_preset_skin_info(app, PresetRole::Steve)?;
      textures.insert(
        0,
        Texture {
          texture_type: TextureType::Skin,
          image: load_image_from_dir(&skin_path)
            .ok_or(AccountError::TextureError)?
            .into(),
          model,
          preset: Some(PresetRole::Steve),
        },
      );
    }
    Ok(textures)
  }

  async fn upload_skin(
    app: &AppHandle,
    player: &PlayerInfo,
    file_path: &Path,
    model: SkinModel,
  ) -> SJMCLResult<MinecraftProfile> {
    let client = app.state::<reqwest::Client>();
    let variant = if model == SkinModel::Slim {
      "slim"
    } else {
      "classic"
    };
    let file = fs::read(file_path).map_err(|_| AccountError::TextureError)?;
    let form = Form::new().text("variant", variant).part(
      "file",
      Part::bytes(file)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|_| AccountError::TextureError)?,
    );

    let response = client
      .post("https://api.minecraftservices.com/minecraft/profile/skins")
      .bearer_auth(player.access_token.clone().unwrap_or_default())
      .multipart(form)
      .send()
      .await
      .map_err(|_| AccountError::NetworkError)?;

    if !response.status().is_success() {
      log::error!("Failed to upload skin: {}", response.status());
      return Err(AccountError::NoTextureApi.into());
    }

    Ok(
      response
        .json::<MinecraftProfile>()
        .await
        .map_err(|_| AccountError::ParseError)?,
    )
  }

  async fn upload_cape(
    _app: &AppHandle,
    _player: &PlayerInfo,
    _file_path: &Path,
  ) -> SJMCLResult<MinecraftProfile> {
    // Minecraft services API does not offer cape uploading for players
    Err(AccountError::NoTextureApi.into())
  }

  async fn delete_cape(app: &AppHandle, player: &PlayerInfo) -> SJMCLResult<MinecraftProfile> {
    send_cape_request(app, player, build_cape_request(None)).await
  }
}

/// One ACTIVE texture of a profile whose image still has to be downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTexture {
  pub texture_type: TextureType,
  pub url: String,
  pub model: SkinModel,
}

/// List the ACTIVE textures of a profile that the launcher has to download, skin first.
pub fn parse_texture_entries(profile: &MinecraftProfile) -> Vec<PendingTexture> {
  let mut entries = Vec::new();
  if let Some(skins) = &profile.skins {
    for skin in skins.iter().filter(|skin| skin.state == ACTIVE_STATE) {
      entries.push(PendingTexture {
        texture_type: TextureType::Skin,
        url: skin.url.clone(),
        model: skin.variant.clone().unwrap_or_default(),
      });
    }
  }
  if let Some(capes) = &profile.capes {
    for cape in capes.iter().filter(|cape| cape.state == ACTIVE_STATE) {
      entries.push(PendingTexture {
        texture_type: TextureType::Cape,
        url: cape.url.clone(),
        model: SkinModel::Default,
      });
    }
  }
  entries
}

/// Build the texture list from the downloaded images.
///
/// A texture whose image could not be downloaded is skipped instead of failing the whole login
/// or token refresh: one broken texture host must not force the user to log in again. Only a
/// warning is logged, and the textures that did download are kept.
pub fn assemble_textures(
  downloads: Vec<(PendingTexture, SJMCLResult<ImageWrapper>)>,
) -> Vec<Texture> {
  downloads
    .into_iter()
    .filter_map(|(pending, download)| match download {
      Ok(image) => Some(Texture {
        texture_type: pending.texture_type,
        image,
        model: pending.model,
        preset: None,
      }),
      Err(error) => {
        log::warn!(
          "Skipping the {} texture from {}: {error:?}",
          pending.texture_type,
          pending.url
        );
        None
      }
    })
    .collect()
}

const ACTIVE_STATE: &str = "ACTIVE";

/// One cape of a Minecraft profile, as returned by the Mojang API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapeEntry {
  pub id: String,
  pub alias: String,
  pub url: String,
  pub active: bool,
}

/// The HTTP request that selects (PUT) or clears (DELETE) the active cape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapeRequest {
  pub method: reqwest::Method,
  pub url: &'static str,
  pub body: Option<String>,
}

/// List every cape of the profile, skipping entries the API returned without an id.
pub fn parse_cape_entries(profile: &MinecraftProfile) -> Vec<CapeEntry> {
  profile
    .capes
    .as_deref()
    .unwrap_or_default()
    .iter()
    .filter_map(|cape| {
      let id = cape.id.clone().filter(|id| !id.trim().is_empty())?;
      Some(CapeEntry {
        id,
        alias: cape.alias.clone().unwrap_or_default(),
        url: cape.url.clone(),
        active: cape.state == ACTIVE_STATE,
      })
    })
    .collect()
}

/// Build the cape list from the downloaded images.
///
/// A cape whose texture could not be downloaded stays in the list with an empty image, so the
/// user still sees which capes exist and can switch to them; only the picture is missing. One
/// failing texture host must never hide the whole cape list.
pub fn assemble_player_capes(
  downloads: Vec<(CapeEntry, SJMCLResult<ImageWrapper>)>,
) -> Vec<PlayerCapeInfo> {
  downloads
    .into_iter()
    .map(|(cape, download)| {
      let image = match download {
        Ok(image) => image,
        Err(error) => {
          log::warn!(
            "Skipping the texture of cape {} from {}: {error:?}",
            cape.id,
            cape.url
          );
          ImageWrapper::default()
        }
      };
      PlayerCapeInfo {
        id: cape.id,
        alias: cape.alias,
        image,
        active: cape.active,
      }
    })
    .collect()
}

/// Build the request that selects a cape id, or clears the active cape when it is blank.
pub fn build_cape_request(cape_id: Option<&str>) -> CapeRequest {
  let cape_id = cape_id.map(str::trim).filter(|id| !id.is_empty());
  match cape_id {
    Some(id) => CapeRequest {
      method: reqwest::Method::PUT,
      url: ACTIVE_CAPE_ENDPOINT,
      body: Some(json!({ "capeId": id }).to_string()),
    },
    None => CapeRequest {
      method: reqwest::Method::DELETE,
      url: ACTIVE_CAPE_ENDPOINT,
      body: None,
    },
  }
}

async fn send_cape_request(
  app: &AppHandle,
  player: &PlayerInfo,
  request: CapeRequest,
) -> SJMCLResult<MinecraftProfile> {
  let client = app.state::<reqwest::Client>();
  let mut builder = client
    .request(request.method, request.url)
    .header("Accept", "application/json")
    .bearer_auth(player.access_token.clone().unwrap_or_default());

  if let Some(body) = request.body {
    builder = builder
      .header("Content-Type", "application/json")
      .body(body);
  }

  let response = builder
    .send()
    .await
    .map_err(|_| AccountError::NetworkError)?;
  let status = response.status();
  if !status.is_success() {
    log::error!("Failed to update the active cape: {}", status);
    let error = if status == reqwest::StatusCode::UNAUTHORIZED {
      AccountError::Expired
    } else if status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::NOT_FOUND
    {
      AccountError::Invalid
    } else {
      AccountError::NoTextureApi
    };
    return Err(error.into());
  }

  Ok(
    response
      .json::<MinecraftProfile>()
      .await
      .map_err(|_| AccountError::ParseError)?,
  )
}

impl MicrosoftTextureOperation {
  /// Fetch every cape of the player's Minecraft profile, including its texture image.
  pub async fn fetch_player_capes(
    app: &AppHandle,
    player: &PlayerInfo,
  ) -> SJMCLResult<Vec<PlayerCapeInfo>> {
    let profile =
      fetch_minecraft_profile(app, player.access_token.clone().unwrap_or_default()).await?;

    let mut downloads = Vec::new();
    for cape in parse_cape_entries(&profile) {
      let image = fetch_image(app, cape.url.clone()).await;
      downloads.push((cape, image));
    }
    Ok(assemble_player_capes(downloads))
  }

  /// Select the cape with the given id, or clear the active cape when it is None.
  pub async fn set_active_cape(
    app: &AppHandle,
    player: &PlayerInfo,
    cape_id: Option<String>,
  ) -> SJMCLResult<MinecraftProfile> {
    match cape_id
      .map(|cape_id| cape_id.trim().to_string())
      .filter(|cape_id| !cape_id.is_empty())
    {
      Some(cape_id) => send_cape_request(app, player, build_cape_request(Some(&cape_id))).await,
      // Clearing the cape is exactly the explicit "no cape" action of the UI.
      None => MicrosoftTextureOperation::delete_cape(app, player).await,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::account::helpers::microsoft::constants::ACTIVE_CAPE_ENDPOINT;
  use crate::account::helpers::microsoft::models::MinecraftProfile;
  use crate::account::helpers::microsoft::texture::{
    CapeEntry, PendingTexture, assemble_player_capes, assemble_textures, build_cape_request,
    parse_cape_entries, parse_texture_entries,
  };
  use crate::account::models::{AccountError, SkinModel, TextureType};
  use crate::utils::image::ImageWrapper;
  use image::{Rgba, RgbaImage};
  use tauri_plugin_http::reqwest::Method;

  fn parse_profile(json: &str) -> MinecraftProfile {
    serde_json::from_str(json).expect("test profile should deserialize")
  }

  #[test]
  fn active_cape_endpoint_matches_the_mojang_api() {
    assert_eq!(
      ACTIVE_CAPE_ENDPOINT,
      "https://api.minecraftservices.com/minecraft/profile/capes/active"
    );
  }

  #[test]
  fn cape_request_for_some_id_uses_put_with_cape_id_body() {
    let request = build_cape_request(Some("migrator"));
    assert_eq!(request.method, Method::PUT);
    assert_eq!(request.url, ACTIVE_CAPE_ENDPOINT);

    let body: serde_json::Value =
      serde_json::from_str(request.body.as_deref().expect("PUT has a body"))
        .expect("body should be JSON");
    assert_eq!(body["capeId"], "migrator");
  }

  #[test]
  fn cape_request_for_none_uses_delete_without_body() {
    let request = build_cape_request(None);
    assert_eq!(request.method, Method::DELETE);
    assert_eq!(request.url, ACTIVE_CAPE_ENDPOINT);
    assert_eq!(request.body, None);
  }

  #[test]
  fn cape_request_for_blank_id_uses_delete_without_body() {
    for blank in ["", "   ", "\t"] {
      let request = build_cape_request(Some(blank));
      assert_eq!(request.method, Method::DELETE);
      assert_eq!(request.body, None);
    }
  }

  #[test]
  fn cape_request_trims_the_cape_id() {
    let request = build_cape_request(Some("  mapmaker \n"));
    assert_eq!(request.method, Method::PUT);

    let body: serde_json::Value =
      serde_json::from_str(request.body.as_deref().expect("PUT has a body"))
        .expect("body should be JSON");
    assert_eq!(body["capeId"], "mapmaker");
  }

  #[test]
  fn cape_entries_from_profile_json_keep_alias_and_active_flag() {
    let profile = parse_profile(
      r#"{
        "id": "069a79f444e94726a5befca90e38aaf5",
        "name": "Notch",
        "skins": [{ "state": "ACTIVE", "url": "https://textures.minecraft.net/texture/skin", "variant": "CLASSIC" }],
        "capes": [
          { "id": "cape-migrator", "state": "ACTIVE", "url": "https://textures.minecraft.net/texture/cape-1", "alias": "Migrator" },
          { "id": "cape-mapmaker", "state": "INACTIVE", "url": "https://textures.minecraft.net/texture/cape-2" },
          { "state": "INACTIVE", "url": "https://textures.minecraft.net/texture/cape-3", "alias": "No id" }
        ]
      }"#,
    );

    let capes = parse_cape_entries(&profile);
    assert_eq!(capes.len(), 2);
    assert_eq!(capes[0].id, "cape-migrator");
    assert_eq!(capes[0].alias, "Migrator");
    assert!(capes[0].active);
    assert_eq!(capes[1].id, "cape-mapmaker");
    assert_eq!(capes[1].alias, "");
    assert!(!capes[1].active);
  }

  #[test]
  fn profile_without_capes_has_no_cape_entries() {
    let profile = parse_profile(r#"{ "id": "069a79f444e94726a5befca90e38aaf5", "name": "Notch" }"#);

    assert!(parse_cape_entries(&profile).is_empty());
  }

  fn pending_texture(texture_type: TextureType, url: &str) -> PendingTexture {
    PendingTexture {
      texture_type,
      url: url.to_string(),
      model: SkinModel::Default,
    }
  }

  fn cape_entry(id: &str, alias: &str, active: bool) -> CapeEntry {
    CapeEntry {
      id: id.to_string(),
      alias: alias.to_string(),
      url: format!("https://textures.minecraft.net/texture/{id}"),
      active,
    }
  }

  fn texture_image(pixel: [u8; 4]) -> ImageWrapper {
    RgbaImage::from_pixel(2, 2, Rgba(pixel)).into()
  }

  #[test]
  fn active_texture_entries_keep_the_skin_first() {
    let profile = parse_profile(
      r#"{
        "id": "069a79f444e94726a5befca90e38aaf5",
        "name": "Notch",
        "skins": [
          { "state": "ACTIVE", "url": "https://textures.minecraft.net/texture/skin", "variant": "SLIM" },
          { "state": "INACTIVE", "url": "https://textures.minecraft.net/texture/old-skin" }
        ],
        "capes": [
          { "id": "cape-migrator", "state": "ACTIVE", "url": "https://textures.minecraft.net/texture/cape-1" },
          { "id": "cape-mapmaker", "state": "INACTIVE", "url": "https://textures.minecraft.net/texture/cape-2" }
        ]
      }"#,
    );

    let entries = parse_texture_entries(&profile);

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].texture_type, TextureType::Skin);
    assert_eq!(entries[0].model, SkinModel::Slim);
    assert_eq!(entries[1].texture_type, TextureType::Cape);
    assert_eq!(entries[1].model, SkinModel::Default);
  }

  #[test]
  fn one_failed_texture_download_does_not_fail_the_profile() {
    // The cape host answers 500: the skin texture still has to reach the caller, so the login
    // and the token refresh survive a broken texture download.
    let downloads = vec![
      (
        pending_texture(
          TextureType::Skin,
          "https://textures.minecraft.net/texture/skin",
        ),
        Ok(texture_image([1, 2, 3, 255])),
      ),
      (
        pending_texture(
          TextureType::Cape,
          "https://textures.minecraft.net/texture/cape",
        ),
        Err(AccountError::NetworkError.into()),
      ),
    ];

    let textures = assemble_textures(downloads);

    assert_eq!(textures.len(), 1);
    assert_eq!(textures[0].texture_type, TextureType::Skin);
    assert_eq!(textures[0].image.image.dimensions(), (2, 2));
  }

  #[test]
  fn failed_texture_downloads_are_skipped_without_an_error() {
    let downloads = vec![
      (
        pending_texture(
          TextureType::Skin,
          "https://textures.minecraft.net/texture/skin",
        ),
        Err(AccountError::Expired.into()),
      ),
      (
        pending_texture(
          TextureType::Cape,
          "https://textures.minecraft.net/texture/cape",
        ),
        Err(AccountError::NetworkError.into()),
      ),
    ];

    assert!(assemble_textures(downloads).is_empty());
  }

  #[test]
  fn failed_cape_texture_keeps_the_cape_entry_with_an_empty_image() {
    let downloads = vec![
      (
        cape_entry("cape-migrator", "Migrator", true),
        Ok(texture_image([4, 5, 6, 255])),
      ),
      (
        cape_entry("cape-mapmaker", "", false),
        Err(AccountError::NetworkError.into()),
      ),
    ];

    let capes = assemble_player_capes(downloads);

    assert_eq!(capes.len(), 2);
    assert_eq!(capes[0].id, "cape-migrator");
    assert!(capes[0].active);
    assert_eq!(capes[0].image.image.dimensions(), (2, 2));
    assert_eq!(capes[1].id, "cape-mapmaker");
    assert_eq!(capes[1].image, ImageWrapper::default());
    assert_eq!(capes[1].image.image.dimensions(), (0, 0));
    assert!(!capes[1].active);
  }

  #[test]
  fn a_failing_cape_texture_does_not_drop_the_cape_list() {
    let downloads = vec![
      (
        cape_entry("cape-migrator", "Migrator", true),
        Err(AccountError::Expired.into()),
      ),
      (
        cape_entry("cape-mapmaker", "", false),
        Err(AccountError::NetworkError.into()),
      ),
    ];

    let capes = assemble_player_capes(downloads);

    assert_eq!(capes.len(), 2);
    assert!(
      capes
        .iter()
        .all(|cape| cape.image == ImageWrapper::default())
    );
    assert!(capes[0].active);
    assert_eq!(capes[0].alias, "Migrator");
    assert_eq!(capes[1].id, "cape-mapmaker");
  }
}
