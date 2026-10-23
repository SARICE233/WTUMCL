use async_trait::async_trait;
use config::Config;
use serde::{Deserialize, Serialize};
use sjmcl_types::error::SJMCLResult;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use zip::ZipArchive;

use crate::instance::helpers::modpack::export::{
  ExportModpackOptions, ModpackExportBundle, normalize_mod_loader_version,
};
use crate::instance::helpers::modpack::import::{
  ModpackManifest, ModpackMetaInfo, locate_pack_file, non_empty, parent_prefix, read_pack_text,
  resolve_mod_loader,
};
use crate::instance::models::misc::{Instance, InstanceError, ModLoaderType};
use crate::resource::models::OtherResourceSource;
use crate::tasks::PTaskParam;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MultiMcCacheRequires {
  pub uid: String,
  pub equals: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MultiMcComponent {
  pub cached_name: Option<String>,
  pub cached_requires: Option<Vec<MultiMcCacheRequires>>,
  pub cached_version: Option<String>,
  pub cached_volatile: Option<bool>,
  pub important: Option<bool>,
  pub dependency_only: Option<bool>,
  pub uid: String,
  pub version: Option<String>,
}

structstruck::strike! {
#[strikethrough[derive(Deserialize, Serialize, Debug, Clone)]]
#[strikethrough[serde(rename_all = "camelCase")]]
  pub struct MultiMcManifest {
    pub components: Vec<MultiMcComponent>,
    pub format_version: u64,
    #[serde(skip)]
    pub cfg: HashMap<String, String>,
    #[serde(skip)]
    pub base_path: String,
    #[serde(skip)]
    pub overrides_path: String,
  }
}

#[async_trait]
impl ModpackManifest for MultiMcManifest {
  fn from_archive(file: &File) -> SJMCLResult<Self> {
    let mut archive = ZipArchive::new(file)?;
    let manifest_path = locate_pack_file(&mut archive, "mmc-pack.json")
      .ok_or(InstanceError::ModpackManifestParseError)?;
    let base_path = parent_prefix(&manifest_path);

    let manifest_content = read_pack_text(&mut archive, &manifest_path)?;
    let mut manifest: Self = serde_json::from_str(&manifest_content)?;

    // Some packs ship without instance.cfg; the pack is still installable, only the
    // display name falls back to a default.
    let cfg_path = format!("{}instance.cfg", base_path);
    let cfg_str = read_pack_text(&mut archive, &cfg_path).unwrap_or_default();
    let cfg = if cfg_str.trim().is_empty() {
      HashMap::new()
    } else {
      Config::builder()
        .add_source(config::File::from_str(&cfg_str, config::FileFormat::Ini))
        .build()?
        .try_deserialize::<HashMap<String, String>>()?
    };

    // PCL/Prism prefer "minecraft/" and only use ".minecraft/" when the former is absent.
    let minecraft_prefix = format!("{}minecraft/", base_path);
    let legacy_prefix = format!("{}.minecraft/", base_path);
    let has_minecraft = archive
      .file_names()
      .flatten()
      .any(|name| name.replace('\\', "/").starts_with(&minecraft_prefix));
    let overrides_path = if has_minecraft {
      minecraft_prefix
    } else {
      legacy_prefix
    };

    manifest.base_path = base_path;
    manifest.cfg = cfg;
    manifest.overrides_path = overrides_path;

    Ok(manifest)
  }

  async fn get_meta_info(&self, app: &AppHandle) -> SJMCLResult<ModpackMetaInfo> {
    let client_version = self.get_client_version()?;
    let mod_loader = match self.get_mod_loader_type_version() {
      Ok((loader_type, version)) => {
        Some(resolve_mod_loader(app, loader_type, version, client_version.clone()).await)
      }
      Err(_) => None,
    };
    Ok(ModpackMetaInfo {
      name: self
        .cfg
        .get("name")
        .and_then(|name| non_empty(name))
        .unwrap_or_else(|| "MultiMC Modpack".to_string()),
      version: None,
      description: None,
      author: None,
      modpack_source: OtherResourceSource::MultiMc,
      client_version,
      mod_loader,
    })
  }

  fn get_client_version(&self) -> SJMCLResult<String> {
    let component = self
      .components
      .iter()
      .find(|component| component.uid == "net.minecraft")
      .ok_or(InstanceError::ModpackManifestParseError)?;

    get_version(component)
  }

  fn get_mod_loader_type_version(&self) -> SJMCLResult<(ModLoaderType, String)> {
    for component in &self.components {
      match component.uid.as_str() {
        "net.minecraft" => continue,
        "net.minecraftforge" => return Ok((ModLoaderType::Forge, get_version(component)?)),
        "net.fabricmc.fabric-loader" => {
          return Ok((ModLoaderType::Fabric, get_version(component)?));
        }
        "net.neoforged" => return Ok((ModLoaderType::NeoForge, get_version(component)?)),
        "org.quiltmc.quilt-loader" => return Ok((ModLoaderType::Quilt, get_version(component)?)),
        _ => continue,
      }
    }
    Err(InstanceError::ModpackManifestParseError.into())
  }

  async fn get_download_params(
    &self,
    _app: &AppHandle,
    _instance_path: &Path,
  ) -> SJMCLResult<Vec<PTaskParam>> {
    // MultiMC Manifests do not include download parameters
    Ok(Vec::new())
  }

  fn get_overrides_path(&self) -> String {
    if self.overrides_path.is_empty() {
      format!("{}.minecraft/", self.base_path)
    } else {
      self.overrides_path.clone()
    }
  }
}

fn get_version(component: &MultiMcComponent) -> SJMCLResult<String> {
  component
    .version
    .as_ref()
    .or(component.cached_version.as_ref())
    .and_then(|version| non_empty(version))
    .ok_or(InstanceError::ModpackManifestParseError.into())
}

pub fn build_multimc_export_bundle(
  instance: &Instance,
  options: &ExportModpackOptions,
  selected_files: &[(String, PathBuf)],
) -> SJMCLResult<ModpackExportBundle> {
  let manifest = generate_multimc_manifest(instance);
  let json = serde_json::to_string_pretty(&manifest)
    .map_err(|_| InstanceError::ModpackManifestParseError)?;

  Ok(ModpackExportBundle {
    overrides_prefix: ".minecraft".to_string(),
    overrides_files: selected_files.to_vec(),
    extra_files: vec![
      ("mmc-pack.json".to_string(), json),
      (
        "instance.cfg".to_string(),
        generate_multimc_instance_cfg(options),
      ),
      (".packignore".to_string(), String::new()),
    ],
  })
}

fn generate_multimc_manifest(instance: &Instance) -> MultiMcManifest {
  let mut components = Vec::new();
  components.push(MultiMcComponent {
    uid: "net.minecraft".to_string(),
    version: Some(instance.version.clone()),
    important: Some(true),
    dependency_only: Some(false),
    cached_name: None,
    cached_requires: None,
    cached_version: None,
    cached_volatile: None,
  });

  if instance.mod_loader.loader_type != ModLoaderType::Unknown {
    let version = normalize_mod_loader_version(&instance.mod_loader.version);
    let uid = match instance.mod_loader.loader_type {
      ModLoaderType::Forge | ModLoaderType::LegacyForge => "net.minecraftforge",
      ModLoaderType::NeoForge => "net.neoforged",
      ModLoaderType::Fabric => "net.fabricmc.fabric-loader",
      ModLoaderType::Quilt => "org.quiltmc.quilt-loader",
      _ => "",
    };

    if !uid.is_empty() {
      components.push(MultiMcComponent {
        uid: uid.to_string(),
        version: Some(version),
        important: Some(false),
        dependency_only: Some(false),
        cached_name: None,
        cached_requires: None,
        cached_version: None,
        cached_volatile: None,
      });
    }
  }

  MultiMcManifest {
    format_version: 1,
    components,
    cfg: HashMap::new(),
    base_path: String::new(),
    overrides_path: String::new(),
  }
}

fn generate_multimc_instance_cfg(options: &ExportModpackOptions) -> String {
  let mut content = format!(
    "# Auto generated by SJMC Launcher\nInstanceType=OneSix\nname={}\n",
    options.name
  );
  if let Some(min_memory) = options.min_memory {
    content.push_str("OverrideMemory=true\n");
    content.push_str(&format!("MinMemAlloc={}\n", min_memory));
  }
  content
}
