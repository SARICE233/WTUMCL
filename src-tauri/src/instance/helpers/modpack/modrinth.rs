use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha1::Digest;
use sjmcl_macros::serialize_skip_none;
use sjmcl_types::error::SJMCLResult;
use smart_default::SmartDefault;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Semaphore;
use zip::ZipArchive;

use crate::instance::helpers::modpack::export::{
  ExportModpackOptions, ModpackExportBundle, normalize_mod_loader_version,
};
use crate::instance::helpers::modpack::import::{
  ModpackManifest, ModpackMetaInfo, locate_pack_file, non_empty, parent_prefix, read_pack_text,
  resolve_mod_loader, safe_relative_path,
};
use crate::instance::models::misc::{Instance, InstanceError, ModLoader, ModLoaderType};
use crate::resource::helpers::{
  curseforge::fetch_remote_resource_by_local_curseforge,
  modrinth::fetch_remote_resource_by_local_modrinth,
};
use crate::resource::models::OtherResourceSource;
use crate::tasks::PTaskParam;
use crate::tasks::download::DownloadParam;

fn default_env_value() -> String {
  "required".to_string()
}

fn default_format_version() -> u64 {
  1
}

fn default_game() -> String {
  "minecraft".to_string()
}

structstruck::strike! {
#[strikethrough[serialize_skip_none]]
#[strikethrough[derive(Deserialize, Serialize, Debug, Clone)]]
#[strikethrough[serde(rename_all = "camelCase")]]
pub struct ModrinthFile {
  pub path: String,
  pub hashes: Option<pub struct ModrinthFileHashes {
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub sha512: String,
  }>,
  pub env: Option<pub struct ModrinthFileEnv {
    #[serde(default = "default_env_value")]
    pub client: String,
    #[serde(default = "default_env_value")]
    pub server: String,
  }>,
  #[serde(default)]
  pub downloads: Vec<String>,
  #[serde(default)]
  pub file_size: u64,
}
}

#[serialize_skip_none]
#[derive(Deserialize, Serialize, Debug, Clone, SmartDefault)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthManifest {
  #[default = 1]
  #[serde(default = "default_format_version")]
  pub format_version: u64,
  #[default = "minecraft"]
  #[serde(default = "default_game")]
  pub game: String,
  #[serde(default)]
  pub version_id: String,
  #[serde(default)]
  pub name: String,
  pub summary: Option<String>,
  #[serde(default)]
  pub files: Vec<ModrinthFile>,
  #[serde(default)]
  pub dependencies: HashMap<String, String>,
  #[serde(skip)]
  pub base_path: String,
}

impl ModrinthManifest {
  fn display_name(&self) -> String {
    non_empty(&self.name)
      .or_else(|| non_empty(&self.version_id))
      .unwrap_or_else(|| "Modrinth Modpack".to_string())
  }

  /// Build metadata without any network access, so format recognition never depends
  /// on remote loader metadata being reachable.
  pub(crate) fn to_meta_info(&self) -> SJMCLResult<ModpackMetaInfo> {
    let client_version = self.get_client_version()?;
    let mod_loader = self
      .get_mod_loader_type_version()
      .ok()
      .map(|(loader_type, version)| ModLoader {
        loader_type,
        version,
        ..Default::default()
      });

    Ok(ModpackMetaInfo {
      name: self.display_name(),
      version: non_empty(&self.version_id),
      description: self.summary.as_deref().and_then(non_empty),
      author: None,
      modpack_source: OtherResourceSource::Modrinth,
      client_version,
      mod_loader,
    })
  }

  /// Translate the manifest file list into download tasks, skipping entries that
  /// cannot be downloaded safely (unsupported client env, no usable URL, or a path
  /// that would escape the instance directory).
  pub(crate) fn download_params_from_files(&self, instance_path: &Path) -> Vec<PTaskParam> {
    self
      .files
      .iter()
      .filter_map(|file| {
        if file
          .env
          .as_ref()
          .is_some_and(|env| env.client.eq_ignore_ascii_case("unsupported"))
        {
          return None;
        }

        let dest = safe_relative_path(&file.path)?;
        let src = file.downloads.iter().find_map(|download| {
          url::Url::parse(download)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https"))
        })?;

        Some(PTaskParam::Download(DownloadParam {
          src,
          sha1: file
            .hashes
            .as_ref()
            .and_then(|hashes| non_empty(&hashes.sha1)),
          dest: instance_path.join(dest),
          filename: None,
        }))
      })
      .collect()
  }
}

#[async_trait]
impl ModpackManifest for ModrinthManifest {
  fn from_archive(file: &File) -> SJMCLResult<Self> {
    let mut archive = ZipArchive::new(file)?;
    let manifest_path = locate_pack_file(&mut archive, "modrinth.index.json")
      .ok_or(InstanceError::ModpackManifestParseError)?;
    let manifest_content = read_pack_text(&mut archive, &manifest_path)?;
    let mut manifest: Self = serde_json::from_str(&manifest_content)?;
    manifest.base_path = parent_prefix(&manifest_path);
    Ok(manifest)
  }

  async fn get_meta_info(&self, app: &AppHandle) -> SJMCLResult<ModpackMetaInfo> {
    let mut meta = self.to_meta_info()?;
    if let Some(loader) = meta.mod_loader.take() {
      meta.mod_loader = Some(
        resolve_mod_loader(
          app,
          loader.loader_type,
          loader.version,
          meta.client_version.clone(),
        )
        .await,
      );
    }
    Ok(meta)
  }

  fn get_client_version(&self) -> SJMCLResult<String> {
    self
      .dependencies
      .get("minecraft")
      .and_then(|version| non_empty(version))
      .ok_or_else(|| InstanceError::ModpackManifestParseError.into())
  }

  fn get_mod_loader_type_version(&self) -> SJMCLResult<(ModLoaderType, String)> {
    // Check loader keys in a fixed order instead of relying on HashMap iteration,
    // which otherwise lets an unrelated dependency mask a supported loader.
    const LOADER_KEYS: [(&str, ModLoaderType); 6] = [
      ("forge", ModLoaderType::Forge),
      ("neoforge", ModLoaderType::NeoForge),
      ("neo-forge", ModLoaderType::NeoForge),
      ("fabric-loader", ModLoaderType::Fabric),
      ("fabric", ModLoaderType::Fabric),
      ("quilt-loader", ModLoaderType::Quilt),
    ];

    for (key, loader_type) in LOADER_KEYS {
      if let Some(version) = self
        .dependencies
        .get(key)
        .and_then(|value| non_empty(value))
      {
        return Ok((loader_type, version));
      }
    }
    Err(InstanceError::ModpackManifestParseError.into())
  }

  async fn get_download_params(
    &self,
    _app: &AppHandle,
    instance_path: &Path,
  ) -> SJMCLResult<Vec<PTaskParam>> {
    Ok(self.download_params_from_files(instance_path))
  }

  fn get_overrides_path(&self) -> String {
    format!("{}overrides/", self.base_path)
  }

  fn get_extra_overrides_paths(&self) -> Vec<String> {
    vec![format!("{}client-overrides/", self.base_path)]
  }
}

pub async fn build_modrinth_export_bundle(
  app: &AppHandle,
  instance: &Instance,
  options: &ExportModpackOptions,
  selected_files: &[(String, PathBuf)],
) -> SJMCLResult<ModpackExportBundle> {
  let mut manifest = generate_modrinth_manifest(instance, options);
  let no_create_remote_files = options.no_create_remote_files.unwrap_or(false);
  let skip_curseforge = options.skip_curseforge_remote_files.unwrap_or(false);
  let (modrinth_files, override_files) =
    collect_modrinth_files(app, selected_files, no_create_remote_files, skip_curseforge).await?;

  manifest.files = modrinth_files;
  let json = serde_json::to_string_pretty(&manifest)
    .map_err(|_| InstanceError::ModpackManifestParseError)?;

  Ok(ModpackExportBundle {
    overrides_prefix: "overrides".to_string(),
    overrides_files: override_files,
    extra_files: vec![("modrinth.index.json".to_string(), json)],
  })
}

fn generate_modrinth_manifest(
  instance: &Instance,
  options: &ExportModpackOptions,
) -> ModrinthManifest {
  let mut dependencies = HashMap::new();
  dependencies.insert("minecraft".to_string(), instance.version.clone());

  if instance.mod_loader.loader_type != ModLoaderType::Unknown {
    let version = normalize_mod_loader_version(&instance.mod_loader.version);
    let loader_key = match instance.mod_loader.loader_type {
      ModLoaderType::Forge | ModLoaderType::LegacyForge => "forge",
      ModLoaderType::Fabric => "fabric-loader",
      ModLoaderType::NeoForge => "neoforge",
      ModLoaderType::Quilt => "quilt-loader",
      _ => "",
    };

    if !loader_key.is_empty() {
      dependencies.insert(loader_key.to_string(), version);
    }
  }

  ModrinthManifest {
    version_id: options.version.clone(),
    name: options.name.clone(),
    summary: options.description.clone(),
    files: Vec::new(),
    dependencies,
    ..Default::default()
  }
}

async fn build_modrinth_remote_file(
  app: &AppHandle,
  rel: &str,
  full: &Path,
  skip_curseforge: bool,
) -> SJMCLResult<Option<ModrinthFile>> {
  let is_disabled = rel.ends_with(".disabled");
  let manifest_path = if is_disabled {
    rel.trim_end_matches(".disabled").to_string()
  } else {
    rel.to_string()
  };

  let mut downloads = Vec::new();

  if let Ok(remote) =
    fetch_remote_resource_by_local_modrinth(app, full.to_string_lossy().as_ref()).await
  {
    downloads.push(remote.download_url);
  }

  if !skip_curseforge
    && let Ok(remote) =
      fetch_remote_resource_by_local_curseforge(app, full.to_string_lossy().as_ref()).await
  {
    downloads.push(remote.download_url);
  }

  if downloads.is_empty() {
    return Ok(None);
  }

  let file_content = tokio::fs::read(full).await?;
  let mut sha1_hasher = sha1::Sha1::new();
  let mut sha512_hasher = sha2::Sha512::new();
  sha1_hasher.update(&file_content);
  sha512_hasher.update(&file_content);
  let sha1 = hex::encode(sha1_hasher.finalize());
  let sha512 = hex::encode(sha512_hasher.finalize());
  let file_size = file_content.len() as u64;
  let env = if is_disabled {
    Some(ModrinthFileEnv {
      client: "optional".to_string(),
      server: "unsupported".to_string(),
    })
  } else {
    None
  };

  Ok(Some(ModrinthFile {
    path: manifest_path,
    hashes: Some(ModrinthFileHashes { sha1, sha512 }),
    env,
    downloads,
    file_size,
  }))
}

async fn collect_modrinth_files(
  app: &AppHandle,
  selected_files: &[(String, PathBuf)],
  no_create_remote_files: bool,
  skip_curseforge: bool,
) -> SJMCLResult<(Vec<ModrinthFile>, Vec<(String, PathBuf)>)> {
  if no_create_remote_files {
    return Ok((Vec::new(), selected_files.to_vec()));
  }

  let is_remote_candidate = |rel: &str| {
    rel.starts_with("mods/") || rel.starts_with("resourcepacks/") || rel.starts_with("shaderpacks/")
  };

  let mut tasks = Vec::new();
  let semaphore = Arc::new(Semaphore::new(
    std::thread::available_parallelism().unwrap().into(),
  ));

  for (rel, full) in selected_files {
    let rel = rel.clone();
    let full = full.clone();
    let app = app.clone();
    let permit = semaphore
      .clone()
      .acquire_owned()
      .await
      .map_err(|_| InstanceError::SemaphoreAcquireFailed)?;

    let task = tokio::spawn({
      let rel = rel.clone();
      let full = full.clone();

      async move {
        let result = if is_remote_candidate(&rel) {
          build_modrinth_remote_file(&app, &rel, &full, skip_curseforge)
            .await
            .ok()
            .flatten()
        } else {
          None
        };

        drop(permit);
        result
      }
    });

    tasks.push((rel, full, task));
  }

  let mut modrinth_files = Vec::new();
  let mut override_files = Vec::new();

  for (rel, full, task) in tasks {
    match task.await {
      Ok(Some(modrinth_file)) => {
        modrinth_files.push(modrinth_file);
      }
      Ok(None) | Err(_) => {
        override_files.push((rel, full));
      }
    }
  }

  Ok((modrinth_files, override_files))
}
