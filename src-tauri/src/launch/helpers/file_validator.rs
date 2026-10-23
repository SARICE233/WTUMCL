use futures::future::join_all;
use futures::stream::{self, StreamExt, TryStreamExt};
use semver::Version;
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use sjmcl_types::storage::load_json_async;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use tauri::AppHandle;
use tokio::fs;
use zip::ZipArchive;

#[cfg(target_os = "windows")]
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use tauri::Manager;

use crate::instance::helpers::asset_index::AssetIndex;
use crate::instance::helpers::asset_index::load_asset_index;
use crate::instance::helpers::client_json::{
  DownloadsArtifact, FeaturesInfo, IsAllowed, LibrariesValue, McClientInfo,
};
use crate::instance::models::misc::{InstanceError, ModLoaderType};
use crate::launch::helpers::misc::get_natives_string;
use crate::launch::models::LaunchError;
use crate::resource::helpers::misc::{convert_url_to_target_source, get_download_api};
use crate::resource::models::{ResourceType, SourceType};
use crate::tasks::PTaskParam;
use crate::tasks::download::DownloadParam;
use crate::utils::fs::validate_sha1;

#[cfg(target_os = "windows")]
use crate::instance::helpers::client_json::load_native_libraries_replace_map;
#[cfg(target_os = "windows")]
use crate::launcher_config::helpers::graphics::mesa_driver_name;
#[cfg(target_os = "windows")]
use crate::launcher_config::models::{GameConfig, LauncherConfig};

pub fn get_nonnative_library_artifacts(client_info: &McClientInfo) -> Vec<DownloadsArtifact> {
  let mut artifacts = HashSet::new();
  let feature = FeaturesInfo::default();
  for library in &client_info.libraries {
    if !library.is_allowed(&feature).unwrap_or(false) {
      continue;
    }
    if library.natives.is_some() {
      continue;
    }
    if let Some(downloads) = &library.downloads
      && let Some(artifact) = &downloads.artifact
    {
      artifacts.insert(artifact.clone());
    }
  }
  artifacts.into_iter().collect()
}

pub fn get_native_library_artifacts(client_info: &McClientInfo) -> Vec<DownloadsArtifact> {
  let mut artifacts = HashSet::new();
  let feature = FeaturesInfo::default();

  for library in &client_info.libraries {
    if !library.is_allowed(&feature).unwrap_or(false) {
      continue;
    }
    if let Some(natives) = &library.natives {
      if let Some(native) = get_natives_string(natives) {
        if let Some(downloads) = &library.downloads
          && let Some(classifiers) = &downloads.classifiers
          && let Some(artifact) = classifiers.get(&native)
        {
          artifacts.insert(artifact.clone());
        }
      } else {
        println!("natives is None");
      }
    }
  }
  artifacts.into_iter().collect()
}

/// Cheap validity check shared by the file collectors.
///
/// The Normal policy used to accept any file that merely existed, so a zero-byte
/// or truncated leftover from an interrupted download looked complete forever and
/// the game started with a corrupted asset/library set (black screen). Size is a
/// free comparison and catches exactly those leftovers; the Full policy keeps
/// paying for the sha1 comparison.
async fn existing_file_is_valid(
  path: &Path,
  expected_sha1: &str,
  expected_size: i64,
  check_hash: bool,
) -> SJMCLResult<bool> {
  if !fs::try_exists(path).await? {
    return Ok(false);
  }
  if check_hash {
    return Ok(validate_sha1(path.to_path_buf(), expected_sha1.to_string()).is_ok());
  }
  if expected_size <= 0 {
    return Ok(true);
  }
  let Ok(metadata) = fs::metadata(path).await else {
    return Ok(false);
  };
  Ok(metadata.len() == expected_size as u64)
}

pub async fn get_invalid_library_files(
  source: SourceType,
  library_path: &Path,
  client_info: &McClientInfo,
  check_hash: bool,
) -> SJMCLResult<Vec<PTaskParam>> {
  let mut artifacts = Vec::new();
  artifacts.extend(get_native_library_artifacts(client_info));
  artifacts.extend(get_nonnative_library_artifacts(client_info));

  let futs = artifacts.into_iter().map(move |artifact| async move {
    let file_path = library_path.join(&artifact.path);
    let valid =
      existing_file_is_valid(&file_path, &artifact.sha1, artifact.size, check_hash).await?;
    if valid {
      Ok(None)
    } else if artifact.url.is_empty() {
      Err(LaunchError::GameFilesIncomplete.into())
    } else {
      let src = convert_url_to_target_source(
        &url::Url::parse(&artifact.url)?,
        &[
          ResourceType::Libraries,
          ResourceType::FabricMaven,
          ResourceType::ForgeMaven,
          ResourceType::ForgeMavenNew,
          ResourceType::NeoforgeMaven,
        ],
        &source,
      )?;
      Ok(Some(PTaskParam::Download(DownloadParam {
        src,
        dest: file_path,
        filename: None,
        sha1: Some(artifact.sha1.clone()),
        size: u64::try_from(artifact.size).ok().filter(|&size| size > 0),
      })))
    }
  });

  let results: Vec<SJMCLResult<Option<PTaskParam>>> = join_all(futs).await;

  let mut params = Vec::new();
  for r in results {
    if let Some(p) = r? {
      params.push(p);
    }
  }

  Ok(params)
}

#[cfg(target_os = "windows")]
pub fn get_windows_mesa_loader_library(app: &AppHandle) -> SJMCLResult<Option<LibrariesValue>> {
  let all_replace_map = load_native_libraries_replace_map(app)?;
  let (os, arch) = {
    let launcher_config_state = app.state::<Mutex<LauncherConfig>>();
    let cfg = launcher_config_state.lock().unwrap();
    (cfg.basic_info.os_type.clone(), cfg.basic_info.arch.clone())
  };
  let platform_key = format!("{}-{}", os.to_lowercase(), arch.to_lowercase());
  Ok(
    all_replace_map
      .get(platform_key.as_str())
      .and_then(|platform_map| platform_map.get("mesa-loader"))
      .cloned()
      .flatten(),
  )
}

#[cfg(target_os = "windows")]
pub fn get_windows_mesa_loader_path(
  app: &AppHandle,
  library_path: &Path,
) -> SJMCLResult<Option<PathBuf>> {
  Ok(get_windows_mesa_loader_library(app)?.and_then(|lib| {
    lib
      .downloads
      .and_then(|downloads| downloads.artifact)
      .map(|artifact| library_path.join(artifact.path))
  }))
}

#[cfg(target_os = "windows")]
pub async fn get_invalid_windows_mesa_loader_file(
  app: &AppHandle,
  source: SourceType,
  library_path: &Path,
  game_config: &GameConfig,
  check_hash: bool,
) -> SJMCLResult<Vec<PTaskParam>> {
  if mesa_driver_name(
    &game_config.advanced.graphics.api,
    &game_config.advanced.graphics.renderer,
  )
  .is_none()
  {
    return Ok(Vec::new());
  }

  let Some(lib) = get_windows_mesa_loader_library(app)? else {
    return Ok(Vec::new());
  };
  let Some(artifact) = lib.downloads.and_then(|downloads| downloads.artifact) else {
    return Ok(Vec::new());
  };

  let file_path = library_path.join(&artifact.path);
  let exists = fs::try_exists(&file_path).await?;
  if exists && (!check_hash || validate_sha1(file_path.clone(), artifact.sha1.clone()).is_ok()) {
    return Ok(Vec::new());
  }

  let src = convert_url_to_target_source(
    &url::Url::parse(&artifact.url)?,
    &[ResourceType::Libraries],
    &source,
  )?;
  Ok(vec![PTaskParam::Download(DownloadParam {
    src,
    dest: file_path,
    filename: None,
    sha1: Some(artifact.sha1),
    size: u64::try_from(artifact.size).ok().filter(|&size| size > 0),
  })])
}

pub struct LibraryParts {
  pub path: String,
  pub pack_name: String,
  pub pack_version: String,
  pub classifier: Option<String>,
  pub extension: String,
}

pub fn parse_library_name(name: &str, native: Option<String>) -> SJMCLResult<LibraryParts> {
  let parts: Vec<&str> = name.split('@').collect();
  let file_ext = if parts.len() > 1 {
    parts[1].to_string()
  } else {
    "jar".to_string()
  };
  let mut name_split: Vec<String> = parts[0].split(':').map(|s| s.to_string()).collect();
  if name_split.len() < 3 {
    Err(InstanceError::InvalidSourcePath.into())
  } else {
    if let Some(native) = native {
      name_split.push(native);
    }
    let pack_name = name_split[1].clone();
    let pack_version = name_split[2].clone();
    let classifier = if name_split.len() > 3 {
      Some(name_split[3].clone())
    } else {
      None
    };
    let path = name_split[0].replace('.', "/");
    Ok(LibraryParts {
      path,
      pack_name,
      pack_version,
      classifier,
      extension: file_ext,
    })
  }
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct LibraryKey {
  path: String,
  pack_name: String,
  classifier: Option<String>,
  extension: String,
}

// merge two vectors of libraries, remove duplicates by name, keep the one with the highest version. also remove libraries with invalid names
pub fn merge_library_lists(
  libraries_a: &[LibrariesValue],
  libraries_b: &[LibrariesValue],
) -> Vec<LibrariesValue> {
  let mut library_map: HashMap<LibraryKey, LibrariesValue> = HashMap::new();

  for library in libraries_a.iter().chain(libraries_b.iter()) {
    if let Ok(library_parts) = parse_library_name(&library.name, None) {
      let key = LibraryKey {
        path: library_parts.path,
        pack_name: library_parts.pack_name,
        classifier: library_parts.classifier,
        extension: library_parts.extension,
      };

      let new_version = library_parts.pack_version;

      if let Some(existing_library) = library_map.get(&key) {
        let existing_version = parse_library_name(&existing_library.name, None)
          .map(|parts| parts.pack_version)
          .unwrap_or("0.1.0".to_string());

        if parse_sem_version(&new_version) > parse_sem_version(&existing_version) {
          library_map.insert(key, library.clone());
        }
      } else {
        library_map.insert(key, library.clone());
      }
    }
  }

  library_map.into_values().collect()
}

fn parse_sem_version(version: &str) -> Version {
  Version::parse(version).unwrap_or({
    let mut parts = version.split('.').collect::<Vec<_>>();
    while parts.len() < 3 {
      parts.push("0");
    }
    Version::parse(&parts[..3].join(".")).unwrap_or(Version::new(0, 1, 0))
  })
}

pub fn convert_library_name_to_path(name: &str, native: Option<String>) -> SJMCLResult<String> {
  let LibraryParts {
    path,
    pack_name,
    pack_version,
    classifier,
    extension: file_ext,
  } = parse_library_name(name, native)?;

  let file_name = [
    pack_name.clone(),
    pack_version.clone(),
    classifier.unwrap_or_default(),
  ]
  .iter()
  .filter(|s| !s.is_empty())
  .map(|s| s.as_str())
  .collect::<Vec<&str>>()
  .join("-")
    + "."
    + &file_ext;
  Ok(format!("{path}/{pack_name}/{pack_version}/{file_name}"))
}

pub fn get_nonnative_library_paths(
  client_info: &McClientInfo,
  library_path: &Path,
  mod_loader_type: ModLoaderType,
) -> SJMCLResult<Vec<PathBuf>> {
  let mut libraries = Vec::new();
  let feature = FeaturesInfo::default();
  for library in &client_info.libraries {
    if !library.is_allowed(&feature).unwrap_or(false) {
      continue;
    }
    if library.natives.is_some() {
      continue;
    }

    if mod_loader_type == ModLoaderType::Cleanroom {
      // Cleanroom replaces legacy LWJGL 2 libraries, whose versions vary by platform.
      if library.name.starts_with("org.lwjgl.lwjgl:lwjgl:")
        || library.name.starts_with("org.lwjgl.lwjgl:lwjgl_util:")
        || matches!(
          library.name.as_str(),
          "net.java.dev.jna:platform:3.4.0" | "com.ibm.icu:icu4j-core-mojang:51.2"
        )
      {
        continue;
      }
    }

    libraries.push(library.clone());
  }

  libraries = merge_library_lists(&libraries, &[]); // remove duplicates to prevent launch errors
  let mut result = Vec::new();
  for library in libraries {
    result.push(library_path.join(convert_library_name_to_path(&library.name, None)?));
  }
  Ok(result)
}

pub fn get_native_library_paths(
  client_info: &McClientInfo,
  library_path: &Path,
  #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] use_native_glfw: bool,
  #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] use_native_openal: bool,
) -> SJMCLResult<Vec<PathBuf>> {
  let mut result = Vec::new();
  let feature = FeaturesInfo::default();
  for library in &client_info.libraries {
    if !library.is_allowed(&feature).unwrap_or(false) {
      continue;
    }
    if let Some(natives) = &library.natives {
      if let Some(native) = get_natives_string(natives) {
        let path = convert_library_name_to_path(&library.name, Some(native))?;
        #[cfg(target_os = "linux")]
        {
          if use_native_glfw && library.name.to_lowercase().contains("glfw") {
            continue;
          }
          if use_native_openal && library.name.to_lowercase().contains("openal") {
            continue;
          }
        }
        result.push(library_path.join(path));
      } else {
        println!("natives is None");
      }
    }
  }
  Ok(result)
}

fn get_native_library_extract_dir(client_info: &McClientInfo, natives_dir: &Path) -> PathBuf {
  const JAVA_LIBRARY_PATH_PREFIX: &str = "-Djava.library.path=";
  const NATIVES_SUBDIR_PREFIX: &str = "${natives_directory}/";

  let Some(arguments) = &client_info.arguments else {
    return natives_dir.to_path_buf();
  };

  for argument in &arguments.jvm {
    if !argument.rules.is_empty() {
      continue;
    }

    for value in &argument.value {
      let Some(path) = value.strip_prefix(JAVA_LIBRARY_PATH_PREFIX) else {
        continue;
      };

      // Minecraft versions before 26.2-snapshot-1 use ${natives_directory} directly.
      if path == "${natives_directory}" {
        return natives_dir.to_path_buf();
      }

      // Minecraft 26.2-snapshot-1+ may use a subdirectory such as ${natives_directory}/java.
      let Some(relative_path) = path.strip_prefix(NATIVES_SUBDIR_PREFIX) else {
        return natives_dir.to_path_buf();
      };
      let relative_path = Path::new(relative_path);

      if relative_path.as_os_str().is_empty()
        || !relative_path
          .components()
          .all(|component| matches!(component, Component::Normal(_)))
      {
        return natives_dir.to_path_buf();
      }

      return natives_dir.join(relative_path);
    }
  }

  natives_dir.to_path_buf()
}

pub async fn extract_native_libraries(
  client_info: &McClientInfo,
  library_path: &Path,
  natives_dir: &Path,
  use_native_glfw: bool,
  use_native_openal: bool,
) -> SJMCLResult<()> {
  #[cfg(target_os = "linux")]
  if (use_native_glfw || use_native_openal) && natives_dir.exists() {
    fs::remove_dir_all(natives_dir).await?;
  }

  let extract_dir = get_native_library_extract_dir(client_info, natives_dir);
  fs::create_dir_all(&extract_dir).await?;

  let native_libraries = get_native_library_paths(
    client_info,
    library_path,
    use_native_glfw,
    use_native_openal,
  )?;
  let tasks: Vec<tokio::task::JoinHandle<SJMCLResult<()>>> = native_libraries
    .into_iter()
    .map(|library_path| {
      let extract_dir = extract_dir.clone();
      tokio::spawn(async move {
        let file = Cursor::new(fs::read(library_path).await?);
        let mut jar = ZipArchive::new(file)?;
        jar.extract(&extract_dir)?;
        Ok(())
      })
    })
    .collect();

  let results = futures::future::join_all(tasks).await;

  for result in results {
    if let Err(e) = result {
      println!("Error handling artifact: {:?}", e);
      return Err(sjmcl_types::error::SJMCLError::from(e));
    }
  }

  Ok(())
}

pub async fn get_invalid_assets(
  app: &AppHandle,
  client_info: &McClientInfo,
  source: SourceType,
  asset_path: &Path,
  check_hash: bool,
) -> SJMCLResult<Vec<PTaskParam>> {
  let assets_download_api = get_download_api(source, ResourceType::Assets)?;

  let asset_index_path = asset_path.join(format!("indexes/{}.json", client_info.asset_index.id));
  let asset_index = load_asset_index(app, &asset_index_path, &client_info.asset_index).await?;

  let futs = asset_index.objects.into_values().map(|item| {
    let assets_download_api = assets_download_api.clone();
    let base_path = asset_path.to_path_buf();

    async move {
      let path_in_repo = format!("{}/{}", &item.hash[..2], item.hash);
      let dest = base_path.join(format!("objects/{}", path_in_repo));
      let valid = existing_file_is_valid(&dest, &item.hash, item.size, check_hash).await?;

      if valid {
        Ok::<Option<PTaskParam>, sjmcl_types::error::SJMCLError>(None)
      } else {
        let src = assets_download_api
          .join(&path_in_repo)
          .map_err(sjmcl_types::error::SJMCLError::from)?;
        Ok(Some(PTaskParam::Download(DownloadParam {
          src,
          dest,
          filename: None,
          sha1: Some(item.hash.clone()),
          size: u64::try_from(item.size).ok().filter(|&size| size > 0),
        })))
      }
    }
  });

  let results: Vec<SJMCLResult<Option<PTaskParam>>> = join_all(futs).await;

  let mut params = Vec::new();
  for r in results {
    if let Some(p) = r? {
      params.push(p);
    }
  }
  Ok(params)
}

pub async fn prepare_legacy_assets(
  root_dir: &Path,
  assets_dir: &Path,
  assets_index_name: &str,
) -> SJMCLResult<()> {
  let target_roots = match assets_index_name {
    "legacy" => vec![assets_dir.join("virtual/legacy")],
    "pre-1.6" => vec![
      assets_dir.join("virtual/legacy"),
      root_dir.join("resources"),
    ],
    _ => return Ok(()),
  };

  let objects_dir = assets_dir.join("objects");
  let asset_index =
    load_json_async::<AssetIndex>(&assets_dir.join(format!("indexes/{}.json", assets_index_name)))
      .await?;

  stream::iter(asset_index.objects)
    .map(Ok::<_, SJMCLError>)
    .try_for_each_concurrent(None, move |(name, item)| {
      let origin = objects_dir.join(format!("{}/{}", &item.hash[..2], item.hash));
      let targets = target_roots
        .iter()
        .map(|target_root| target_root.join(&name))
        .collect::<Vec<_>>();

      async move {
        if !fs::try_exists(&origin).await? {
          return Ok::<(), SJMCLError>(());
        }

        for target in targets {
          if fs::try_exists(&target).await? {
            continue;
          }
          if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await?;
          }
          fs::copy(&origin, &target).await?;
        }
        Ok(())
      }
    })
    .await?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn unique_path(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    std::env::temp_dir().join(format!(
      "sjmcl-file-validator-{}-{}-{}",
      tag,
      std::process::id(),
      nanos
    ))
  }

  #[tokio::test]
  async fn normal_policy_rejects_zero_byte_and_truncated_leftovers() {
    const EXPECTED: i64 = 4096;
    let path = unique_path("normal");

    // Missing file: still invalid, as before.
    assert!(
      !existing_file_is_valid(&path, "", EXPECTED, false)
        .await
        .unwrap()
    );

    // Zero-byte leftover: the file that used to pass an existence-only check.
    std::fs::write(&path, b"").unwrap();
    assert!(
      !existing_file_is_valid(&path, "", EXPECTED, false)
        .await
        .unwrap()
    );

    // Truncated body.
    std::fs::write(&path, vec![0x5a_u8; 1024]).unwrap();
    assert!(
      !existing_file_is_valid(&path, "", EXPECTED, false)
        .await
        .unwrap()
    );

    // Even a longer file is rejected: the size is part of the contract.
    std::fs::write(&path, vec![0x5a_u8; 5000]).unwrap();
    assert!(
      !existing_file_is_valid(&path, "", EXPECTED, false)
        .await
        .unwrap()
    );

    std::fs::write(&path, vec![0x5a_u8; EXPECTED as usize]).unwrap();
    assert!(
      existing_file_is_valid(&path, "", EXPECTED, false)
        .await
        .unwrap()
    );

    // A manifest entry without a size keeps the historical existence-only check.
    assert!(existing_file_is_valid(&path, "", 0, false).await.unwrap());

    let _ = std::fs::remove_file(&path);
  }

  #[tokio::test]
  async fn full_policy_still_hashes_instead_of_trusting_the_size() {
    let path = unique_path("full");
    std::fs::write(&path, b"0123456789").unwrap();
    let wrong_sha1 = "0000000000000000000000000000000000000000";

    // Same size, wrong content: only the hash check can reject it.
    assert!(
      existing_file_is_valid(&path, wrong_sha1, 10, false)
        .await
        .unwrap()
    );
    assert!(
      !existing_file_is_valid(&path, wrong_sha1, 10, true)
        .await
        .unwrap()
    );

    let _ = std::fs::remove_file(&path);
  }
}
