use serde::Deserialize;
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;

use crate::resource::helpers::misc::version_pack_sort;
use crate::resource::helpers::modrinth::ALL_FILTER;
use crate::resource::models::{
  OtherResourceApiEndpoint, OtherResourceDependency, OtherResourceFileInfo, OtherResourceInfo,
  OtherResourceRequestType, OtherResourceSearchRes, OtherResourceSource, OtherResourceVersionPack,
  ResourceError,
};

pub async fn make_modrinth_request<T, P>(
  client: &reqwest::Client,
  url: &str,
  request_type: OtherResourceRequestType<'_, P>,
) -> SJMCLResult<T>
where
  T: serde::de::DeserializeOwned,
  P: serde::Serialize,
{
  let request_builder = match request_type {
    OtherResourceRequestType::GetWithParams(params) => client.get(url).query(params),
    OtherResourceRequestType::Get => client.get(url),
    OtherResourceRequestType::Post(payload) => client.post(url).json(payload),
  };

  let response = request_builder
    .send()
    .await
    .map_err(|_| ResourceError::NetworkError)?;

  if !response.status().is_success() {
    return Err(ResourceError::NetworkError.into());
  }

  response
    .json::<T>()
    .await
    .map_err(|_| ResourceError::ParseError.into())
}

pub fn get_modrinth_api(
  endpoint: OtherResourceApiEndpoint,
  param: Option<&str>,
) -> SJMCLResult<String> {
  let base_url = "https://api.modrinth.com/v2";

  let url_str = match endpoint {
    OtherResourceApiEndpoint::Search => format!("{}/search", base_url),
    OtherResourceApiEndpoint::VersionPack => {
      let project_id = param.ok_or(ResourceError::ParseError)?;
      format!("{}/project/{}/version", base_url, project_id)
    }
    OtherResourceApiEndpoint::FromLocal => {
      let hash = param.ok_or(ResourceError::ParseError)?;
      format!("{}/version_file/{}", base_url, hash)
    }
    OtherResourceApiEndpoint::ById => {
      let project_id = param.ok_or(ResourceError::ParseError)?;
      format!("{}/project/{}", base_url, project_id)
    }
    OtherResourceApiEndpoint::TranslateDesc => {
      let project_id = param.ok_or(ResourceError::ParseError)?;
      format!(
        "https://mod.mcimirror.top/translate/modrinth/{}",
        project_id
      )
    }
  };

  Ok(url_str)
}

// A unified struct for both search projects and get project by id responses
#[derive(Deserialize, Debug)]
pub struct ModrinthProject {
  #[serde(alias = "id")]
  pub project_id: String,
  pub project_type: String,
  pub slug: String,
  pub title: String,
  pub description: String,
  pub categories: Vec<String>,
  pub downloads: u64,
  pub icon_url: Option<String>,
  #[serde(alias = "updated")]
  pub date_modified: String,
  pub author: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ModrinthSearchRes {
  pub hits: Vec<ModrinthProject>,
  pub total_hits: u64,
  pub offset: u32,
  pub limit: u32,
}

structstruck::strike! {
#[strikethrough[derive(Deserialize, Debug)]]
  pub struct ModrinthFileInfo {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub hashes: pub struct {
      pub sha1: String,
    },
  }
}

structstruck::strike! {
#[strikethrough[derive(Deserialize, Debug)]]
  pub struct ModrinthVersionPack {
    pub project_id: String,
    pub dependencies: Vec<pub struct {
      pub project_id: Option<String>,
      pub dependency_type: String,
    }>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub name: String,
    pub date_published: String,
    pub downloads: u64,
    pub version_type: String,
    pub files: Vec<ModrinthFileInfo>,
  }
}

#[derive(Deserialize, Debug)]
pub struct ModrinthTranslationRes {
  pub translated: String,
}

fn normalize_modrinth_loader(loader: &str) -> Option<String> {
  if loader.is_empty() || loader == "minecraft" {
    None
  } else {
    match loader.to_lowercase().as_str() {
      "forge" => Some("Forge".to_string()),
      "fabric" => Some("Fabric".to_string()),
      "quilt" => Some("Quilt".to_string()),
      "neoforge" => Some("NeoForge".to_string()),
      "vanilla" => Some("Vanilla".to_string()),
      "iris" => Some("Iris".to_string()),
      "canvas" => Some("Canvas".to_string()),
      "optifine" => Some("OptiFine".to_string()),
      _ => Some(loader.to_string()),
    }
  }
}

// Modrinth tags loaders on the version, not on each file. A version that
// declares several loaders but ships several files therefore cannot tell us
// which file is built for which loader; attributing every file to every
// declared loader is what let a Fabric instance be recommended a NeoForge
// build (issue #2033). Such versions expose no loader attribution at all
// instead of a wrong one.
fn modrinth_version_file_loaders(version: &ModrinthVersionPack) -> Vec<String> {
  const ALLOWED_LOADERS: &[&str] = &[
    "forge",
    "fabric",
    "quilt",
    "neoforge",
    "vanilla",
    "iris",
    "canvas",
    "optifine",
    "minecraft",
  ];

  let mut loaders = version
    .loaders
    .iter()
    .filter(|loader| ALLOWED_LOADERS.contains(&loader.as_str()))
    .filter_map(|loader| normalize_modrinth_loader(loader))
    .collect::<Vec<_>>();
  loaders.dedup();

  if loaders.len() > 1 && version.files.len() > 1 {
    return Vec::new();
  }

  loaders
}

pub fn map_modrinth_file_to_version_pack(
  res: Vec<ModrinthVersionPack>,
  resource_type: &str,
  mod_loader: &str,
) -> Vec<OtherResourceVersionPack> {
  let mut version_packs = std::collections::HashMap::new();
  let is_modpack = resource_type == "modpack";
  // None means the "All" filter: keep every loader a file can be attributed
  // to. Otherwise the version itself must declare the requested loader, so a
  // loader-filtered list never carries a build of another loader.
  let requested_loader = if mod_loader == ALL_FILTER {
    None
  } else {
    normalize_modrinth_loader(mod_loader)
  };

  for version in res {
    let game_versions = if version.game_versions.is_empty() {
      vec!["".to_string()]
    } else {
      version.game_versions.clone()
    };

    let file_loaders = modrinth_version_file_loaders(&version);

    let item_loaders: Vec<Option<String>> = if is_modpack {
      vec![None]
    } else {
      match &requested_loader {
        Some(loader) if file_loaders.iter().any(|l| l.eq_ignore_ascii_case(loader)) => {
          vec![Some(loader.clone())]
        }
        Some(_) => continue,
        None if file_loaders.is_empty() => vec![None],
        None => file_loaders.into_iter().map(Some).collect(),
      }
    };

    for game_version in &game_versions {
      if is_modpack {
        let file = version
          .files
          .iter()
          .find(|file| file.primary)
          .or_else(|| version.files.first());

        if let Some(file) = file {
          version_packs
            .entry(game_version.clone())
            .or_insert_with(|| OtherResourceVersionPack {
              name: game_version.clone(),
              items: Vec::new(),
            })
            .items
            .push((&version, file, None).into());
        }
      } else {
        for loader in &item_loaders {
          let file_infos = version
            .files
            .iter()
            .map(|file| (&version, file, loader.clone()).into())
            .collect::<Vec<_>>();

          version_packs
            .entry(game_version.clone())
            .or_insert_with(|| OtherResourceVersionPack {
              name: game_version.clone(),
              items: Vec::new(),
            })
            .items
            .extend(file_infos);
        }
      }
    }
  }

  let mut list: Vec<OtherResourceVersionPack> = version_packs.into_values().collect();
  list.sort_by(version_pack_sort);

  list
}

// Pick the newest stable file whose loader attribution matches the requested
// loader. The recommendation never falls back to a file attributed to another
// loader (issue #2033).
pub fn select_recommended_modrinth_file<'a>(
  version_packs: &'a [OtherResourceVersionPack],
  game_version: &str,
  loader: &str,
) -> Option<&'a OtherResourceFileInfo> {
  version_packs
    .iter()
    .filter(|pack| pack.name == game_version)
    .flat_map(|pack| pack.items.iter())
    .filter(|item| matches!(item.release_type.as_str(), "beta" | "release"))
    .filter(|item| {
      item
        .loader
        .as_deref()
        .is_some_and(|item_loader| item_loader.eq_ignore_ascii_case(loader))
    })
    .max_by(|a, b| a.file_date.cmp(&b.file_date))
}

impl From<ModrinthProject> for OtherResourceInfo {
  fn from(project: ModrinthProject) -> Self {
    Self {
      id: project.project_id,
      mcmod_id: 0,
      _type: project.project_type,
      name: project.title,
      slug: project.slug.to_string(),
      description: project.description,
      icon_src: project.icon_url.unwrap_or_default(),
      website_url: format!("https://modrinth.com/mod/{}", project.slug),
      tags: project.categories,
      last_updated: project.date_modified,
      downloads: project.downloads,
      source: OtherResourceSource::Modrinth,
      translated_name: None,
      translated_description: None,
      author: project.author,
    }
  }
}

impl From<(&ModrinthVersionPack, &ModrinthFileInfo, Option<String>)> for OtherResourceFileInfo {
  fn from(
    (version, file, loader): (&ModrinthVersionPack, &ModrinthFileInfo, Option<String>),
  ) -> Self {
    Self {
      resource_id: version.project_id.clone(),
      name: version.name.clone(),
      release_type: version.version_type.clone(),
      downloads: version.downloads,
      file_date: version.date_published.clone(),
      download_url: file.url.clone(),
      sha1: file.hashes.sha1.clone(),
      file_name: file.filename.clone(),
      dependencies: version
        .dependencies
        .iter()
        .map(|d| OtherResourceDependency {
          resource_id: d.project_id.clone().unwrap_or_default(),
          relation: d.dependency_type.clone(),
        })
        .collect(),
      loader,
    }
  }
}

impl From<ModrinthSearchRes> for OtherResourceSearchRes {
  fn from(res: ModrinthSearchRes) -> Self {
    let list = res.hits.into_iter().map(OtherResourceInfo::from).collect();

    Self {
      list,
      total: res.total_hits,
      page: res.offset / res.limit,
      page_size: res.limit,
    }
  }
}

pub async fn translate_description_modrinth(
  app: &AppHandle,
  resource_id: &str,
) -> SJMCLResult<Option<String>> {
  let result = async {
    let url = get_modrinth_api(OtherResourceApiEndpoint::TranslateDesc, Some(resource_id))?;
    let client = app.state::<reqwest::Client>();

    let translation_res = client
      .get(&url)
      .send()
      .await?
      .json::<ModrinthTranslationRes>()
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

  // Minimal Modrinth data reproducing issue #2033: a NeoForge build and a
  // Fabric build of the same game version, plus a newer version that declares
  // both loaders while shipping one file per loader.
  const ISSUE_2033_VERSIONS: &str = r#"[
    {
      "project_id": "tectonic",
      "dependencies": [],
      "game_versions": ["26.3"],
      "loaders": ["neoforge"],
      "name": "v3.0.31 ~ Neoforge 26.3",
      "date_published": "2026-10-18T00:00:00Z",
      "downloads": 4200,
      "version_type": "release",
      "files": [
        {
          "url": "https://cdn.modrinth.com/data/tectonic/versions/x/tectonic-neoforge-3.0.31.jar",
          "filename": "tectonic-neoforge-3.0.31.jar",
          "primary": true,
          "hashes": { "sha1": "1111111111111111111111111111111111111111" }
        }
      ]
    },
    {
      "project_id": "tectonic",
      "dependencies": [],
      "game_versions": ["26.3"],
      "loaders": ["fabric"],
      "name": "v3.0.31 ~ Fabric 26.3",
      "date_published": "2026-10-16T00:00:00Z",
      "downloads": 4100,
      "version_type": "release",
      "files": [
        {
          "url": "https://cdn.modrinth.com/data/tectonic/versions/y/tectonic-fabric-3.0.31.jar",
          "filename": "tectonic-fabric-3.0.31.jar",
          "primary": true,
          "hashes": { "sha1": "2222222222222222222222222222222222222222" }
        }
      ]
    },
    {
      "project_id": "tectonic",
      "dependencies": [],
      "game_versions": ["26.3"],
      "loaders": ["fabric", "neoforge"],
      "name": "v3.0.32 ~ Fabric/Neoforge 26.3",
      "date_published": "2026-10-22T00:00:00Z",
      "downloads": 4300,
      "version_type": "release",
      "files": [
        {
          "url": "https://cdn.modrinth.com/data/tectonic/versions/z/tectonic-neoforge-3.0.32.jar",
          "filename": "tectonic-neoforge-3.0.32.jar",
          "primary": true,
          "hashes": { "sha1": "3333333333333333333333333333333333333333" }
        },
        {
          "url": "https://cdn.modrinth.com/data/tectonic/versions/w/tectonic-fabric-3.0.32.jar",
          "filename": "tectonic-fabric-3.0.32.jar",
          "primary": false,
          "hashes": { "sha1": "4444444444444444444444444444444444444444" }
        }
      ]
    }
  ]"#;

  fn issue_2033_versions() -> Vec<ModrinthVersionPack> {
    serde_json::from_str(ISSUE_2033_VERSIONS)
      .expect("issue 2033 fixture should match the Modrinth version shape")
  }

  #[test]
  fn modrinth_recommendation_respects_instance_loader() {
    let version_packs = map_modrinth_file_to_version_pack(issue_2033_versions(), "mod", "Fabric");

    let pack = version_packs
      .iter()
      .find(|pack| pack.name == "26.3")
      .expect("a Fabric pack for game version 26.3");
    let fabric_files = pack
      .items
      .iter()
      .map(|item| item.file_name.as_str())
      .collect::<Vec<_>>();
    assert_eq!(fabric_files, vec!["tectonic-fabric-3.0.31.jar"]);
    assert!(
      pack
        .items
        .iter()
        .all(|item| item.loader.as_deref() == Some("Fabric"))
    );

    let recommended = select_recommended_modrinth_file(&version_packs, "26.3", "Fabric")
      .expect("a Fabric recommendation");
    assert_eq!(recommended.file_name, "tectonic-fabric-3.0.31.jar");
  }

  #[test]
  fn modrinth_loader_filter_never_attributes_other_loader_files() {
    let version_packs = map_modrinth_file_to_version_pack(issue_2033_versions(), "mod", "All");

    let fabric_files = version_packs
      .iter()
      .flat_map(|pack| pack.items.iter())
      .filter(|item| item.loader.as_deref() == Some("Fabric"))
      .map(|item| item.file_name.as_str())
      .collect::<Vec<_>>();
    assert_eq!(fabric_files, vec!["tectonic-fabric-3.0.31.jar"]);

    // The mixed-loader version ships one file per loader, so neither file may
    // be claimed by a loader filter.
    for file_name in ["tectonic-neoforge-3.0.32.jar", "tectonic-fabric-3.0.32.jar"] {
      let item = version_packs
        .iter()
        .flat_map(|pack| pack.items.iter())
        .find(|item| item.file_name == file_name)
        .expect("the ambiguous file is still listed for the All filter");
      assert_eq!(item.loader, None);
    }

    let neoforge = select_recommended_modrinth_file(&version_packs, "26.3", "NeoForge")
      .expect("a NeoForge recommendation");
    assert_eq!(neoforge.file_name, "tectonic-neoforge-3.0.31.jar");
  }

  #[test]
  fn modrinth_naive_loader_attribution_would_recommend_wrong_loader() {
    // Negative control: reproduce the previous attribution (every file tagged
    // with every loader its version declares) and show that it recommends a
    // NeoForge build for a Fabric instance, which the current mapping rejects.
    let mut naive_files = Vec::new();
    for version in issue_2033_versions() {
      for loader in &version.loaders {
        for file in &version.files {
          naive_files.push((
            file.filename.clone(),
            normalize_modrinth_loader(loader).unwrap_or_default(),
            version.date_published.clone(),
          ));
        }
      }
    }
    let mut naive_fabric_files = naive_files
      .into_iter()
      .filter(|(_, loader, _)| loader.as_str().eq_ignore_ascii_case("Fabric"))
      .collect::<Vec<_>>();
    naive_fabric_files.sort_by(|a, b| b.2.cmp(&a.2));
    assert_eq!(
      naive_fabric_files.first().map(|(name, _, _)| name.as_str()),
      Some("tectonic-neoforge-3.0.32.jar")
    );

    let version_packs = map_modrinth_file_to_version_pack(issue_2033_versions(), "mod", "All");
    let recommended = select_recommended_modrinth_file(&version_packs, "26.3", "Fabric")
      .expect("a Fabric recommendation");
    assert_ne!(recommended.file_name, "tectonic-neoforge-3.0.32.jar");
  }
}
