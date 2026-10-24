use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use std::collections::HashMap;
use std::env;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;

use crate::resource::helpers::misc::version_pack_sort;
use crate::resource::models::{
  OtherResourceApiEndpoint, OtherResourceDependency, OtherResourceFileInfo, OtherResourceInfo,
  OtherResourceRequestType, OtherResourceSearchRes, OtherResourceSource, OtherResourceVersionPack,
  ResourceError,
};

lazy_static! {
  pub static ref CURSEFORGE_API_KEY: String = {
    env::var("WTUMCL_CURSEFORGE_API_KEY")
      .unwrap_or_else(|_| env!("WTUMCL_CURSEFORGE_API_KEY").to_string())
  };
}

pub fn is_curseforge_authenticated_url(url: &url::Url) -> bool {
  matches!(
    url.host_str(),
    Some("api.curseforge.com" | "edge.forgecdn.net" | "mediafilez.forgecdn.net")
  )
}

/// CurseForge rejects the request without a usable body sometimes, so the reason is
/// trimmed before it is written to the log.
const CURSEFORGE_ERROR_REASON_MAX_LEN: usize = 160;

/// An unset credential is a configuration problem, so it has to be reported before
/// the request is attempted instead of being mistaken for a connection failure.
fn curseforge_credential_error(url: &str, api_key: &str) -> Option<SJMCLError> {
  if api_key.trim().is_empty() {
    log::warn!(
      "CurseForge API {} is skipped: WTUMCL_CURSEFORGE_API_KEY is not configured",
      url
    );
    return Some(ResourceError::CurseForgeApiKeyMissing.into());
  }

  None
}

/// Collapse whitespace and bound the length so an unexpected HTML error page cannot
/// flood the log.
fn sanitize_curseforge_error_reason(reason: &str) -> String {
  let flattened = reason.split_whitespace().collect::<Vec<_>>().join(" ");
  let mut trimmed = flattened.chars();
  let mut bounded = trimmed
    .by_ref()
    .take(CURSEFORGE_ERROR_REASON_MAX_LEN)
    .collect::<String>();
  if trimmed.next().is_some() {
    bounded.push_str("...");
  }

  bounded
}

pub async fn make_curseforge_request<T, P>(
  client: &reqwest::Client,
  url: &str,
  request_type: OtherResourceRequestType<'_, P>,
) -> SJMCLResult<T>
where
  T: serde::de::DeserializeOwned,
  P: serde::Serialize,
{
  make_curseforge_request_with_key(client, url, request_type, CURSEFORGE_API_KEY.as_str()).await
}

/// `make_curseforge_request` with the credential injected, so the three failure modes
/// can be verified without reading process state or reaching the network.
async fn make_curseforge_request_with_key<T, P>(
  client: &reqwest::Client,
  url: &str,
  request_type: OtherResourceRequestType<'_, P>,
  api_key: &str,
) -> SJMCLResult<T>
where
  T: serde::de::DeserializeOwned,
  P: serde::Serialize,
{
  if let Some(error) = curseforge_credential_error(url, api_key) {
    return Err(error);
  }

  let request_builder = match request_type {
    OtherResourceRequestType::GetWithParams(params) => client.get(url).query(params),
    OtherResourceRequestType::Get => client.get(url),
    OtherResourceRequestType::Post(payload) => client.post(url).json(payload),
  };

  let response = request_builder
    .header("x-api-key", api_key)
    .send()
    .await
    .map_err(|error| {
      log::warn!("Failed to connect to CurseForge API {}: {}", url, error);
      ResourceError::NetworkError
    })?;

  let status = response.status();
  if !status.is_success() {
    let reason = response.text().await.unwrap_or_default();
    log::warn!(
      "CurseForge API {} returned {} with body: {}",
      url,
      status,
      sanitize_curseforge_error_reason(&reason)
    );

    // CurseForge answered, so a refused request is a rejection and not a transport
    // failure. Only a client error blames the request; a 5xx is the API being unwell,
    // which is retried like any other connectivity problem.
    return Err(if status.is_client_error() {
      ResourceError::CurseForgeRequestRejected.into()
    } else {
      ResourceError::NetworkError.into()
    });
  }

  response
    .json::<T>()
    .await
    .map_err(|_| ResourceError::ParseError.into())
}

pub fn get_curseforge_api(
  endpoint: OtherResourceApiEndpoint,
  id: Option<&str>,
) -> SJMCLResult<String> {
  let base_url = "https://api.curseforge.com/v1";

  let url_str = match endpoint {
    OtherResourceApiEndpoint::Search => format!("{}/mods/search", base_url),
    OtherResourceApiEndpoint::VersionPack => {
      let mod_id = id.ok_or(ResourceError::ParseError)?;
      format!("{}/mods/{}/files", base_url, mod_id)
    }
    OtherResourceApiEndpoint::FromLocal => format!("{}/fingerprints/432", base_url),
    OtherResourceApiEndpoint::ById => {
      let mod_id = id.ok_or(ResourceError::ParseError)?;
      format!("{}/mods/{}", base_url, mod_id)
    }
    OtherResourceApiEndpoint::TranslateDesc => {
      let mod_id = id.ok_or(ResourceError::ParseError)?;
      format!("https://mod.mcimirror.top/translate/curseforge/{}", mod_id)
    }
  };

  Ok(url_str)
}

structstruck::strike! {
#[strikethrough[derive(Deserialize, Serialize, Debug, Clone)]]
#[strikethrough[serde(rename_all = "camelCase")]]
  pub struct CurseForgeProject {
    pub id: i32,
    pub class_id: Option<i32>,
    pub links: pub struct {
      pub website_url: String,
    },
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub categories: Vec<pub struct {
      pub name: String,
    }>,
    pub download_count: u64,
    pub logo: Option<pub struct {
      pub url: String,
    }>,
    pub date_modified: String,
    pub authors: Vec<pub struct {
      pub name: String,
    }>,
  }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgePagination {
  pub index: u32,
  pub page_size: u32,
  pub total_count: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CurseForgeSearchRes {
  pub data: Vec<CurseForgeProject>,
  pub pagination: CurseForgePagination,
}

structstruck::strike! {
#[strikethrough[derive(Deserialize, Serialize, Debug, Clone)]]
#[strikethrough[serde(rename_all = "camelCase")]]
  pub struct CurseForgeFileInfo {
    pub id: i32,
    pub mod_id: i32,
    pub display_name: String,
    pub file_name: String,
    pub release_type: u32,
    pub hashes: Vec<pub struct {
      pub value: String,
      pub algo: u32,
    }>,
    pub file_date: String,
    pub download_url: Option<String>,
    pub download_count: u64,
    pub game_versions: Vec<String>,
    pub dependencies: Vec<pub struct {
      pub mod_id: i32,
      pub relation_type: u32,
    }>,
  }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CurseForgeVersionPackSearchRes {
  pub data: Vec<CurseForgeFileInfo>,
  pub pagination: CurseForgePagination,
}

structstruck::strike! {
#[strikethrough[derive(Deserialize, Serialize, Debug, Clone)]]
#[strikethrough[serde(rename_all = "camelCase")]]
  pub struct CurseForgeFingerprintRes {
    pub data: pub struct {
      pub exact_matches: Vec<pub struct {
        pub file: CurseForgeFileInfo,
      }>,
    }
  }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeGetProjectRes {
  pub data: CurseForgeProject,
}

#[derive(Deserialize, Debug)]
pub struct CurseForgeTranslationRes {
  pub translated: String,
}

fn extract_versions_and_loaders(game_versions: &[String]) -> (Vec<String>, Vec<String>) {
  let mut versions = Vec::new();
  let mut loaders = Vec::new();

  const ALLOWED_LOADERS: &[&str] = &[
    "Forge", "Fabric", "Quilt", "NeoForge", "Vanilla", "Iris", "Canvas", "OptiFine",
  ];

  for v in game_versions {
    if v.starts_with(|c: char| c.is_ascii_digit()) {
      versions.push(v.clone());
    } else if ALLOWED_LOADERS.contains(&v.as_str()) {
      loaders.push(v.clone());
    }
  }

  (versions, loaders)
}

pub fn map_curseforge_file_to_version_pack(
  res: Vec<CurseForgeFileInfo>,
) -> Vec<OtherResourceVersionPack> {
  let mut version_packs = std::collections::HashMap::new();

  for cf_file in res {
    let (versions, loaders) = extract_versions_and_loaders(&cf_file.game_versions);

    let versions = if versions.is_empty() {
      vec!["".to_string()]
    } else {
      versions
    };

    let loaders = if loaders.is_empty() {
      vec![""]
    } else {
      loaders.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    };

    for version in &versions {
      for loader in &loaders {
        let file_info = (
          &cf_file,
          if loader.is_empty() {
            None
          } else {
            Some(loader.to_string())
          },
        )
          .into();

        version_packs
          .entry(version.clone())
          .or_insert_with(|| OtherResourceVersionPack {
            name: version.clone(),
            items: Vec::new(),
          })
          .items
          .push(file_info);
      }
    }
  }

  let mut list: Vec<OtherResourceVersionPack> = version_packs.into_values().collect();
  list.sort_by(version_pack_sort);

  list
}

impl From<CurseForgeProject> for OtherResourceInfo {
  fn from(project: CurseForgeProject) -> Self {
    Self {
      id: project.id.to_string(),
      mcmod_id: 0,
      _type: cvt_class_id_to_type(project.class_id.unwrap_or(0)),
      name: project.name,
      slug: project.slug,
      description: project.summary,
      icon_src: project.logo.map_or("".to_string(), |logo| logo.url),
      website_url: project.links.website_url,
      tags: project.categories.iter().map(|c| c.name.clone()).collect(),
      last_updated: project.date_modified,
      downloads: project.download_count,
      source: OtherResourceSource::CurseForge,
      translated_name: None,
      translated_description: None,
      author: project.authors.first().map(|author| author.name.clone()),
    }
  }
}

impl From<(&CurseForgeFileInfo, Option<String>)> for OtherResourceFileInfo {
  fn from((cf_file, loader): (&CurseForgeFileInfo, Option<String>)) -> Self {
    Self {
      resource_id: cf_file.mod_id.to_string(),
      name: cf_file.display_name.clone(),
      release_type: cvt_id_to_release_type(cf_file.release_type),
      downloads: cf_file.download_count,
      file_date: cf_file.file_date.clone(),
      download_url: cf_file.download_url.clone().unwrap_or(format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        cf_file.id / 1000,
        cf_file.id % 1000,
        cf_file.file_name.clone()
      )),
      sha1: cf_file
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map_or("".to_string(), |h| h.value.clone()),
      file_name: cf_file.file_name.clone(),
      dependencies: cf_file
        .dependencies
        .iter()
        .map(|dep| OtherResourceDependency {
          resource_id: dep.mod_id.to_string(),
          relation: cvt_id_to_dependency_type(dep.relation_type),
        })
        .collect(),
      loader,
    }
  }
}

impl From<CurseForgeSearchRes> for OtherResourceSearchRes {
  fn from(res: CurseForgeSearchRes) -> Self {
    let list = res.data.into_iter().map(OtherResourceInfo::from).collect();

    Self {
      list,
      total: res.pagination.total_count,
      page: res.pagination.index / res.pagination.page_size,
      page_size: res.pagination.page_size,
    }
  }
}

lazy_static! {
  static ref CATEGORY_MAP: HashMap<(String, u32), u32> = {
    let mut map = HashMap::new();

    // mods
    map.insert(("Food".to_string(), 6), 436);
    map.insert(("Ores and Resources".to_string(), 6), 408);
    map.insert(("Miscellaneous".to_string(), 6), 425);
    map.insert(("Thermal Expansion".to_string(), 6), 427);
    map.insert(("Cosmetic".to_string(), 6), 424);
    map.insert(("Education".to_string(), 6), 5299);
    map.insert(("Buildcraft".to_string(), 6), 432);
    map.insert(("Processing".to_string(), 6), 413);
    map.insert(("Map and Information".to_string(), 6), 423);
    map.insert(("Tinker's Construct".to_string(), 6), 428);
    map.insert(("Industrial Craft".to_string(), 6), 429);
    map.insert(("Technology".to_string(), 6), 412);
    map.insert(("Farming".to_string(), 6), 416);
    map.insert(("Structures".to_string(), 6), 409);
    map.insert(("Genetics".to_string(), 6), 418);
    map.insert(("Magic".to_string(), 6), 419);
    map.insert(("Addons".to_string(), 6), 426);
    map.insert(("Dimensions".to_string(), 6), 410);
    map.insert(("Mobs".to_string(), 6), 411);
    map.insert(("Armor, Tools, and Weapons".to_string(), 6), 434);
    map.insert(("Server Utility".to_string(), 6), 435);
    map.insert(("Energy, Fluid, and Item Transport".to_string(), 6), 415);
    map.insert(("World Gen".to_string(), 6), 406);
    map.insert(("Player Transport".to_string(), 6), 414);
    map.insert(("Forestry".to_string(), 6), 433);
    map.insert(("Applied Energistics 2".to_string(), 6), 4545);
    map.insert(("Energy".to_string(), 6), 417);
    map.insert(("Adventure and RPG".to_string(), 6), 422);
    map.insert(("Storage".to_string(), 6), 420);
    map.insert(("Biomes".to_string(), 6), 407);
    map.insert(("Redstone".to_string(), 6), 4558);
    map.insert(("Thaumcraft".to_string(), 6), 430);
    map.insert(("Blood Magic".to_string(), 6), 4485);
    map.insert(("API and Library".to_string(), 6), 421);
    map.insert(("Twitch Integration".to_string(), 6), 4671);
    map.insert(("Automation".to_string(), 6), 4843);
    map.insert(("CraftTweaker".to_string(), 6), 4773);
    map.insert(("MCreator".to_string(), 6), 4906);
    map.insert(("KubeJS".to_string(), 6), 5314);
    map.insert(("Utility & QoL".to_string(), 6), 5191);
    map.insert(("Galacticraft".to_string(), 6), 5232);
    map.insert(("Skyblock".to_string(), 6), 6145);
    map.insert(("Create".to_string(), 6), 6484);
    map.insert(("Integrated Dynamics".to_string(), 6), 6954);
    map.insert(("Performance".to_string(), 6), 6814);
    map.insert(("Bug Fixes".to_string(), 6), 6821);
    map.insert(("Twilight Forest".to_string(), 6), 7669);

    // resource packs
    map.insert(("Photo Realistic".to_string(), 12), 400);
    map.insert(("Steampunk".to_string(), 12), 399);
    map.insert(("Traditional".to_string(), 12), 403);
    map.insert(("512x and Higher".to_string(), 12), 398);
    map.insert(("128x".to_string(), 12), 396);
    map.insert(("256x".to_string(), 12), 397);
    map.insert(("64x".to_string(), 12), 395);
    map.insert(("Medieval".to_string(), 12), 402);
    map.insert(("Miscellaneous".to_string(), 12), 405);
    map.insert(("32x".to_string(), 12), 394);
    map.insert(("16x".to_string(), 12), 393);
    map.insert(("Animated".to_string(), 12), 404);
    map.insert(("Modern".to_string(), 12), 401);
    map.insert(("Mod Support".to_string(), 12), 4465);
    map.insert(("Data Packs".to_string(), 12), 5193);
    map.insert(("Font Packs".to_string(), 12), 5244);

    // worlds
    map.insert(("Parkour".to_string(), 17), 251);
    map.insert(("Survival".to_string(), 17), 253);
    map.insert(("Creation".to_string(), 17), 249);
    map.insert(("Game Map".to_string(), 17), 250);
    map.insert(("Adventure".to_string(), 17), 248);
    map.insert(("Modded World".to_string(), 17), 4464);
    map.insert(("Puzzle".to_string(), 17), 252);

    // mod packs
    map.insert(("Adventure and RPG".to_string(), 4471), 4475);
    map.insert(("FTB Official Pack".to_string(), 4471), 4487);
    map.insert(("Small / Light".to_string(), 4471), 4481);
    map.insert(("Quests".to_string(), 4471), 4478);
    map.insert(("Combat / PvP".to_string(), 4471), 4483);
    map.insert(("Tech".to_string(), 4471), 4472);
    map.insert(("Sci-Fi".to_string(), 4471), 4474);
    map.insert(("Multiplayer".to_string(), 4471), 4484);
    map.insert(("Hardcore".to_string(), 4471), 4479);
    map.insert(("Mini Game".to_string(), 4471), 4477);
    map.insert(("Extra Large".to_string(), 4471), 4482);
    map.insert(("Magic".to_string(), 4471), 4473);
    map.insert(("Skyblock".to_string(), 4471), 4736);
    map.insert(("Map Based".to_string(), 4471), 4480);
    map.insert(("Exploration".to_string(), 4471), 4476);
    map.insert(("Vanilla+".to_string(), 4471), 5128);
    map.insert(("Horror".to_string(), 4471), 7418);

    // shader packs
    map.insert(("Vanilla".to_string(), 6552), 6555);
    map.insert(("Fantasy".to_string(), 6552), 6554);
    map.insert(("Realistic".to_string(), 6552), 6553);

    // data packs
    map.insert(("Magic".to_string(), 6945), 6952);
    map.insert(("Miscellaneous".to_string(), 6945), 6947);
    map.insert(("Fantasy".to_string(), 6945), 6949);
    map.insert(("Mod Support".to_string(), 6945), 6946);
    map.insert(("Tech".to_string(), 6945), 6951);
    map.insert(("Library".to_string(), 6945), 6950);
    map.insert(("Utility".to_string(), 6945), 6953);
    map.insert(("Adventure".to_string(), 6945), 6948);
    map
  };
}

pub fn cvt_category_to_id(category: &str, class_id: u32) -> u32 {
  match CATEGORY_MAP.get(&(category.to_string(), class_id)) {
    Some(id) => *id,
    None => {
      println!(
        "Category not found: {} with class_id {}",
        category, class_id
      );
      0
    }
  }
}

pub fn cvt_class_id_to_type(class_id: i32) -> String {
  match class_id {
    6 => "mod".to_string(),
    12 => "resourcepack".to_string(),
    17 => "world".to_string(),
    4471 => "modpack".to_string(),
    6552 => "shader".to_string(),
    6945 => "datapack".to_string(),
    _ => "unknown".to_string(),
  }
}

pub fn cvt_type_to_class_id(_type: &str) -> u32 {
  match _type {
    "mod" => 6,
    "resourcepack" => 12,
    "world" => 17,
    "modpack" => 4471,
    "shader" => 6552,
    "datapack" => 6945,
    _ => 0,
  }
}

pub fn cvt_sort_by_to_id(sort_by: &str) -> u32 {
  match sort_by {
    "Popularity" => 2,
    "A-Z" => 4,
    "Latest update" => 3,
    "Creation date" => 11,
    "Total downloads" => 6,
    _ => 2,
  }
}

pub fn cvt_mod_loader_to_id(mod_loader: &str) -> u32 {
  match mod_loader {
    "Forge" => 1,
    "Fabric" => 4,
    "Quilt" => 5,
    "NeoForge" => 6,
    _ => 0,
  }
}

pub fn cvt_id_to_release_type(release_type: u32) -> String {
  match release_type {
    1 => "release".to_string(),
    2 => "beta".to_string(),
    _ => "alpha".to_string(),
  }
}

pub fn cvt_id_to_dependency_type(dependency_type: u32) -> String {
  match dependency_type {
    1 => "embedded".to_string(),
    2 => "optional".to_string(),
    3 => "required".to_string(),
    4 => "tool".to_string(),
    5 => "incompatible".to_string(),
    _ => "include".to_string(),
  }
}

pub async fn translate_description_curseforge(
  app: &AppHandle,
  resource_id: &str,
) -> SJMCLResult<Option<String>> {
  let result = async {
    let url = get_curseforge_api(OtherResourceApiEndpoint::TranslateDesc, Some(resource_id))?;
    let client = app.state::<reqwest::Client>();

    let translation_res = client
      .get(&url)
      .header("x-api-key", CURSEFORGE_API_KEY.as_str())
      .send()
      .await?
      .json::<CurseForgeTranslationRes>()
      .await?;

    Ok::<_, SJMCLError>(translation_res.translated)
  }
  .await;

  // Only return Ok(None) when translation fails, to avoid blocking major functionality
  Ok(result.ok())
}
#[cfg(test)]
mod tests {
  use super::*;
  use std::io::{Read, Write};

  const TRANSPORT_ERROR: &str = "NETWORK_ERROR";
  const MISSING_KEY_ERROR: &str = "CURSE_FORGE_API_KEY_MISSING";
  const REJECTED_ERROR: &str = "CURSE_FORGE_REQUEST_REJECTED";

  /// Answers one request on loopback only, so the rejection path is exercised offline.
  fn spawn_curseforge_stub(status_line: &str, body: &str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let response = format!(
      "{}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
      status_line,
      body.len(),
      body
    );

    std::thread::spawn(move || {
      let mut chunk = [0u8; 4096];
      let mut received = Vec::new();
      for stream in listener.incoming() {
        let Ok(mut stream) = stream else { break };
        while !received.windows(4).any(|w| w == b"\r\n\r\n") {
          match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(size) => received.extend_from_slice(&chunk[..size]),
          }
        }
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        break;
      }
    });

    format!("http://127.0.0.1:{}/v1/mods/search", port)
  }

  async fn error_for(client: &reqwest::Client, url: &str, api_key: &str) -> SJMCLError {
    make_curseforge_request_with_key::<serde_json::Value, ()>(
      client,
      url,
      OtherResourceRequestType::Get,
      api_key,
    )
    .await
    .unwrap_err()
  }

  #[test]
  fn the_three_failure_modes_have_three_distinct_names() {
    assert_eq!(
      ResourceError::CurseForgeApiKeyMissing.to_string(),
      MISSING_KEY_ERROR
    );
    assert_eq!(
      ResourceError::CurseForgeRequestRejected.to_string(),
      REJECTED_ERROR
    );
    assert_eq!(ResourceError::NetworkError.to_string(), TRANSPORT_ERROR);

    let names = [MISSING_KEY_ERROR, REJECTED_ERROR, TRANSPORT_ERROR];
    for (i, a) in names.iter().enumerate() {
      for (j, b) in names.iter().enumerate() {
        if i != j {
          assert_ne!(*a, *b, "failure kinds must stay distinguishable");
        }
      }
    }
  }

  #[test]
  fn credential_guard_flags_blank_keys_only() {
    for blank in ["", " ", "\t", "\r\n"] {
      assert_eq!(
        curseforge_credential_error("/v1/mods/search", blank),
        Some(SJMCLError(MISSING_KEY_ERROR.to_string())),
        "key {:?} must be reported as an unset credential",
        blank
      );
    }

    assert_eq!(
      curseforge_credential_error("/v1/mods/search", "$2a$10$abcdef"),
      None
    );
  }

  #[tokio::test]
  async fn an_unset_credential_stops_before_any_connection_is_attempted() {
    let client = reqwest::Client::new();

    // Unusable URL on purpose: had the blank key been ignored the failure would have
    // come back as a transport error, which is the collapse being fixed.
    let unset = error_for(&client, "not-a-valid-url", "").await;
    let configured = error_for(&client, "not-a-valid-url", "configured-key").await;

    assert_eq!(unset, SJMCLError(MISSING_KEY_ERROR.to_string()));
    assert_eq!(configured, SJMCLError(TRANSPORT_ERROR.to_string()));
    assert_ne!(unset, configured);
  }

  #[tokio::test]
  async fn a_forbidden_response_is_a_rejection_and_not_a_network_error() {
    let url = spawn_curseforge_stub(
      "HTTP/1.1 403 Forbidden",
      "Forbidden: API Key missing or invalid",
    );
    let client = reqwest::Client::new();

    let rejected = error_for(&client, &url, "configured-key").await;

    assert_eq!(rejected, SJMCLError(REJECTED_ERROR.to_string()));
    assert_ne!(rejected, SJMCLError(TRANSPORT_ERROR.to_string()));
    assert_ne!(rejected, SJMCLError(MISSING_KEY_ERROR.to_string()));
  }

  #[tokio::test]
  async fn a_successful_response_still_deserializes() {
    let url = spawn_curseforge_stub("HTTP/1.1 200 OK", r#"{"count": 1}"#);
    let client = reqwest::Client::new();

    let body: serde_json::Value = make_curseforge_request_with_key::<_, ()>(
      &client,
      &url,
      OtherResourceRequestType::Get,
      "configured-key",
    )
    .await
    .unwrap();

    assert_eq!(body["count"], 1);
  }

  #[tokio::test]
  async fn every_refused_client_request_is_a_rejection() {
    for status_line in [
      "HTTP/1.1 401 Unauthorized",
      "HTTP/1.1 403 Forbidden",
      "HTTP/1.1 404 Not Found",
    ] {
      let url = spawn_curseforge_stub(status_line, "Forbidden: API Key missing or invalid");
      let client = reqwest::Client::new();

      assert_eq!(
        error_for(&client, &url, "configured-key").await,
        SJMCLError(REJECTED_ERROR.to_string()),
        "{} must be reported as a rejection",
        status_line
      );
    }
  }

  #[tokio::test]
  async fn a_server_error_stays_a_network_error() {
    let url = spawn_curseforge_stub("HTTP/1.1 500 Internal Server Error", "upstream panicked");
    let client = reqwest::Client::new();

    assert_eq!(
      error_for(&client, &url, "configured-key").await,
      SJMCLError(TRANSPORT_ERROR.to_string())
    );
  }

  #[tokio::test]
  async fn unset_credential_wins_over_a_refusal_from_the_same_url() {
    let url = spawn_curseforge_stub(
      "HTTP/1.1 403 Forbidden",
      "Forbidden: API Key missing or invalid",
    );
    let client = reqwest::Client::new();

    assert_eq!(
      error_for(&client, &url, "").await,
      SJMCLError(MISSING_KEY_ERROR.to_string())
    );
  }

  #[test]
  fn response_reason_is_flattened_and_bounded() {
    let noisy = "Forbidden:\n   API Key\tmissing\r\n or invalid ".repeat(30);
    let sanitized = sanitize_curseforge_error_reason(&noisy);

    assert!(!sanitized.contains('\n'));
    assert!(!sanitized.contains('\t'));
    assert!(
      sanitized.ends_with("..."),
      "overlong body must be truncated"
    );
    assert_eq!(
      sanitized.chars().count(),
      CURSEFORGE_ERROR_REASON_MAX_LEN + 3
    );

    assert_eq!(
      sanitize_curseforge_error_reason("  Forbidden: API Key missing or invalid  "),
      "Forbidden: API Key missing or invalid"
    );
    assert_eq!(sanitize_curseforge_error_reason("   \n  "), "");
  }
}
