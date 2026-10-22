use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sjmcl_types::error::{SJMCLError, SJMCLResult};
use std::fs;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};
use tauri::AppHandle;
use zip::ZipArchive;

use crate::instance::helpers::modpack::curseforge::CurseForgeManifest;
use crate::instance::helpers::modpack::modrinth::ModrinthManifest;
use crate::instance::helpers::modpack::multimc::MultiMcManifest;
use crate::instance::models::misc::{InstanceError, ModLoader, ModLoaderType};
use crate::resource::commands::fetch_mod_loader_version_list;
use crate::resource::models::OtherResourceSource;
use crate::tasks::PTaskParam;

// `async_trait` expands the async methods into `#[must_use]` boxed futures;
// clippy's double_must_use has no actionable fix in the trait source.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait ModpackManifest {
  fn from_archive(file: &File) -> SJMCLResult<Self>
  where
    Self: Sized;
  fn get_client_version(&self) -> SJMCLResult<String>;
  fn get_mod_loader_type_version(&self) -> SJMCLResult<(ModLoaderType, String)>;
  async fn get_meta_info(&self, app: &AppHandle) -> SJMCLResult<ModpackMetaInfo>;
  async fn get_download_params(
    &self,
    app: &AppHandle,
    instance_path: &Path,
  ) -> SJMCLResult<Vec<PTaskParam>>;
  fn get_overrides_path(&self) -> String;
  /// Extra override directories that are applied after get_overrides_path
  /// (for example Modrinth's optional client-overrides). Defaults to none.
  fn get_extra_overrides_paths(&self) -> Vec<String> {
    Vec::new()
  }
}

type ManifestBox = Box<dyn ModpackManifest + Send + Sync>;
type Parser = Box<dyn Fn(&File) -> SJMCLResult<ManifestBox> + Send + Sync>;

fn get_parsers() -> Vec<Parser> {
  vec![
    Box::new(|f| {
      CurseForgeManifest::from_archive(f).map(|m| {
        let b: ManifestBox = Box::new(m);
        b
      })
    }),
    Box::new(|f| {
      ModrinthManifest::from_archive(f).map(|m| {
        let b: ManifestBox = Box::new(m);
        b
      })
    }),
    Box::new(|f| {
      MultiMcManifest::from_archive(f).map(|m| {
        let b: ManifestBox = Box::new(m);
        b
      })
    }),
  ]
}

/// Locate a top-level pack manifest by content, tolerating a single extra directory
/// layer (some repacks wrap the whole pack in a folder) and case differences.
///
/// Mirrors the archive-base-folder sniffing done by PCL/PCL2-CE: first the archive
/// root, then entries one directory deep.
pub(crate) fn locate_pack_file<R: Read + Seek>(
  archive: &mut ZipArchive<R>,
  file_name: &str,
) -> Option<String> {
  if archive.by_name(file_name).is_ok() {
    return Some(file_name.to_string());
  }

  let mut candidates: Vec<String> = Vec::new();
  for name in archive.file_names() {
    let normalized = name.replace('\\', "/");
    let normalized = normalized.trim_start_matches("./");
    let parts: Vec<&str> = normalized
      .split('/')
      .filter(|part| !part.is_empty())
      .collect();
    if !parts.is_empty()
      && parts.len() <= 2
      && parts
        .last()
        .is_some_and(|last| last.eq_ignore_ascii_case(file_name))
    {
      candidates.push(normalized.to_string());
    }
  }
  candidates.sort();
  candidates.into_iter().next()
}

/// Parent directory (with trailing slash) of a path returned by locate_pack_file.
pub(crate) fn parent_prefix(path: &str) -> String {
  match path.rfind('/') {
    Some(index) => path[..=index].to_string(),
    None => String::new(),
  }
}

pub(crate) fn read_pack_text<R: Read + Seek>(
  archive: &mut ZipArchive<R>,
  path: &str,
) -> SJMCLResult<String> {
  let mut entry = archive
    .by_name(path)
    .map_err(|_| InstanceError::ModpackManifestParseError)?;
  let mut content = String::new();
  entry.read_to_string(&mut content)?;
  Ok(content)
}

pub(crate) fn non_empty(value: &str) -> Option<String> {
  let trimmed = value.trim();
  (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Reject absolute paths and parent-directory components coming from an untrusted
/// manifest. Returns None when the path could escape the instance directory.
pub(crate) fn safe_relative_path(path: &str) -> Option<PathBuf> {
  let normalized = path.replace('\\', "/");
  let mut relative = PathBuf::new();

  for component in Path::new(&normalized).components() {
    match component {
      Component::Normal(part) => relative.push(part),
      Component::CurDir => {}
      Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
    }
  }

  (!relative.as_os_str().is_empty()).then_some(relative)
}

/// Try to enrich a loader version with its branch. A failed lookup must not block
/// recognizing the pack: the loader version itself is authoritative and the branch
/// is only a display/install hint.
pub(crate) async fn resolve_mod_loader(
  app: &AppHandle,
  loader_type: ModLoaderType,
  version: String,
  client_version: String,
) -> ModLoader {
  let loader = ModLoader {
    loader_type,
    version,
    ..Default::default()
  };
  match loader.clone().with_branch(app, client_version).await {
    Ok(resolved) => resolved,
    Err(error) => {
      log::warn!("Failed to resolve mod loader branch: {error:?}");
      loader
    }
  }
}

fn detect_manifest(file: &File) -> SJMCLResult<ManifestBox> {
  if let Err(error) = ZipArchive::new(file) {
    return Err(SJMCLError(format!(
      "{}: {error}",
      InstanceError::ZipFileProcessFailed
    )));
  }

  for parser in get_parsers() {
    if let Ok(manifest) = parser(file) {
      return Ok(manifest);
    }
  }

  Err(InstanceError::ModpackManifestParseError.into())
}

impl ModLoader {
  pub async fn with_branch(&self, app: &AppHandle, mc_version: String) -> SJMCLResult<Self> {
    let version_list =
      fetch_mod_loader_version_list(app.clone(), mc_version, self.loader_type).await?;
    if let Some(version) = version_list.iter().find(|v| v.version == self.version) {
      return Ok(Self {
        branch: version.branch.clone(),
        ..self.clone()
      });
    }
    Err(InstanceError::ModLoaderVersionParseError.into())
  }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModpackMetaInfo {
  pub name: String,
  pub version: Option<String>,
  pub description: Option<String>,
  pub author: Option<String>,
  pub modpack_source: OtherResourceSource,
  pub client_version: String,
  pub mod_loader: Option<ModLoader>,
}

impl ModpackMetaInfo {
  pub async fn from_archive(app: &AppHandle, file: &File) -> SJMCLResult<Self> {
    detect_manifest(file)?.get_meta_info(app).await
  }
}

pub async fn get_download_params(
  app: &AppHandle,
  file: &File,
  instance_path: &Path,
) -> SJMCLResult<Vec<PTaskParam>> {
  detect_manifest(file)?
    .get_download_params(app, instance_path)
    .await
}

pub fn extract_overrides(file: &File, instance_path: &Path) -> SJMCLResult<()> {
  let manifest = detect_manifest(file)?;
  let mut prefixes = vec![manifest.get_overrides_path()];
  prefixes.extend(manifest.get_extra_overrides_paths());
  prefixes.retain(|prefix| !prefix.is_empty());

  let mut archive = ZipArchive::new(file)?;
  // Apply directories in order so extra ones (e.g. client-overrides) win.
  for prefix in &prefixes {
    for i in 0..archive.len() {
      let mut entry = archive.by_index(i)?;
      let Some(relative_path) = entry.enclosed_name() else {
        continue;
      };
      if !entry.is_file() {
        continue;
      }

      let entry_path = relative_path.to_string_lossy().replace('\\', "/");
      let Some(stripped) = entry_path.strip_prefix(prefix.as_str()) else {
        continue;
      };
      let Some(relative) = safe_relative_path(stripped) else {
        continue;
      };

      let outpath = instance_path.join(relative);
      if let Some(parent) = outpath.parent()
        && !parent.exists()
      {
        fs::create_dir_all(parent)?;
      }

      let mut outfile = File::create(&outpath)?;
      std::io::copy(&mut entry, &mut outfile)?;
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::instance::helpers::modpack::curseforge::CurseForgeManifest;
  use crate::instance::helpers::modpack::modrinth::ModrinthManifest;
  use crate::instance::helpers::modpack::multimc::MultiMcManifest;
  use crate::tasks::PTaskParam;
  use std::io::Write;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicUsize, Ordering};

  static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

  struct TempPath(PathBuf);

  impl Drop for TempPath {
    fn drop(&mut self) {
      let _ = std::fs::remove_file(&self.0);
    }
  }

  struct TempDir(PathBuf);

  impl Drop for TempDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  fn unique_path(tag: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
      "sjmcl-modpack-{}-{}-{}.{}",
      tag,
      std::process::id(),
      TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
      extension
    ))
  }

  struct TestArchive {
    file: File,
    // Held only so the temporary archive is removed when the test ends.
    #[allow(dead_code)]
    path: TempPath,
  }

  impl TestArchive {
    fn new(entries: Vec<(&str, Vec<u8>)>) -> Self {
      let path = unique_path("archive", "zip");
      let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
      let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
      for (name, content) in entries {
        writer.start_file(name, options).unwrap();
        writer.write_all(&content).unwrap();
      }
      writer.finish().unwrap();
      let file = File::open(&path).unwrap();
      Self {
        file,
        path: TempPath(path),
      }
    }
  }

  fn make_temp_dir(tag: &str) -> TempDir {
    let path = unique_path(tag, "dir");
    std::fs::create_dir_all(&path).unwrap();
    TempDir(path)
  }

  fn modrinth_manifest(files: &str, dependencies: &str) -> String {
    format!(
      r#"{{"formatVersion":1,"game":"minecraft","versionId":"1.5.2fix2","name":"FarmingTales Sample","summary":"minecraft 1.20.1, Forge 47.4.10","files":{files},"dependencies":{dependencies}}}"#
    )
  }

  #[test]
  fn modrinth_sample_shape_parses_full_meta() {
    let files = r#"[
      {"path":"mods/cloth-config.jar","hashes":{"sha1":"aaa","sha512":"bbb"},"downloads":["https://cdn.example/cloth-config.jar"],"fileSize":1181413},
      {"path":"resourcepacks/pack.zip","hashes":{"sha1":"ccc"},"downloads":["https://cdn.example/pack.zip"]}
    ]"#;
    let archive = TestArchive::new(vec![(
      "modrinth.index.json",
      modrinth_manifest(files, r#"{"minecraft":"1.20.1","forge":"47.4.10"}"#).into_bytes(),
    )]);

    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    assert_eq!(manifest.name, "FarmingTales Sample");
    assert_eq!(manifest.version_id, "1.5.2fix2");
    assert_eq!(manifest.files.len(), 2);
    assert_eq!(manifest.get_client_version().unwrap(), "1.20.1");

    let meta = manifest.to_meta_info().unwrap();
    assert_eq!(meta.name, "FarmingTales Sample");
    assert_eq!(meta.version.as_deref(), Some("1.5.2fix2"));
    assert_eq!(meta.client_version, "1.20.1");
    assert_eq!(meta.modpack_source, OtherResourceSource::Modrinth);
    let loader = meta.mod_loader.expect("forge loader");
    assert_eq!(loader.loader_type, ModLoaderType::Forge);
    assert_eq!(loader.version, "47.4.10");
  }

  #[test]
  fn modrinth_missing_optional_fields_do_not_block_recognition() {
    let archive = TestArchive::new(vec![(
      "modrinth.index.json",
      br#"{"dependencies":{"minecraft":"1.20.1"}}"#.to_vec(),
    )]);

    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    assert!(manifest.files.is_empty());
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.game, "minecraft");

    let meta = manifest.to_meta_info().unwrap();
    assert_eq!(meta.name, "Modrinth Modpack");
    assert_eq!(meta.version, None);
    assert_eq!(meta.description, None);
    assert_eq!(meta.author, None);
    assert_eq!(meta.client_version, "1.20.1");
    assert!(meta.mod_loader.is_none());
  }

  #[test]
  fn modrinth_missing_minecraft_version_is_a_clear_error() {
    let archive = TestArchive::new(vec![(
      "modrinth.index.json",
      br#"{"name":"No MC","dependencies":{"forge":"47.4.10"}}"#.to_vec(),
    )]);

    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    let error = manifest.to_meta_info().unwrap_err();
    assert!(
      error.0.contains("MODPACK_MANIFEST_PARSE_ERROR"),
      "unexpected error: {}",
      error.0
    );
  }

  #[test]
  fn modrinth_unknown_dependency_does_not_mask_known_loader() {
    let archive = TestArchive::new(vec![(
      "modrinth.index.json",
      br#"{"name":"Mixed","dependencies":{"minecraft":"1.20.1","some-future-loader":"9.9","forge":"47.4.10"}}"#.to_vec(),
    )]);

    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    assert_eq!(
      manifest.get_mod_loader_type_version().unwrap(),
      (ModLoaderType::Forge, "47.4.10".to_string())
    );
  }

  #[test]
  fn modrinth_nested_archive_root_and_client_overrides() {
    let dir = make_temp_dir("override");
    let files = r#"[{"path":"mods/a.jar","hashes":{"sha1":"aaa"},"downloads":["https://cdn.example/a.jar"]}]"#;
    let archive = TestArchive::new(vec![
      (
        "Repacked/modrinth.index.json",
        modrinth_manifest(files, r#"{"minecraft":"1.20.1","fabric-loader":"0.15.0"}"#).into_bytes(),
      ),
      (
        "Repacked/overrides/config/test.toml",
        b"from-overrides".to_vec(),
      ),
      ("Repacked/overrides/remove-me.txt", b"stale".to_vec()),
      (
        "Repacked/client-overrides/config/test.toml",
        b"from-client-overrides".to_vec(),
      ),
      (
        "Repacked/client-overrides/client-only.txt",
        b"client".to_vec(),
      ),
      (
        "Repacked/overrides/../escape.txt",
        b"must-not-escape".to_vec(),
      ),
    ]);

    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    assert_eq!(manifest.get_overrides_path(), "Repacked/overrides/");
    assert_eq!(
      manifest.get_extra_overrides_paths(),
      vec!["Repacked/client-overrides/".to_string()]
    );

    super::extract_overrides(&archive.file, &dir.0).unwrap();
    assert_eq!(
      std::fs::read_to_string(dir.0.join("config/test.toml")).unwrap(),
      "from-client-overrides"
    );
    assert!(dir.0.join("remove-me.txt").exists());
    assert!(dir.0.join("client-only.txt").exists());
    assert!(!dir.0.join("escape.txt").exists());
    assert!(!dir.0.parent().unwrap().join("escape.txt").exists());
  }

  #[test]
  fn modrinth_manifest_entry_is_case_insensitive() {
    let archive = TestArchive::new(vec![(
      "SomePack/MODRINTH.INDEX.JSON",
      br#"{"name":"Case","dependencies":{"minecraft":"1.21"}}"#.to_vec(),
    )]);
    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    assert_eq!(manifest.get_client_version().unwrap(), "1.21");
    assert_eq!(manifest.get_overrides_path(), "SomePack/overrides/");
  }

  #[test]
  fn modrinth_download_params_skip_unsupported_and_unsafe_entries() {
    let files = r#"[
      {"path":"mods/good.jar","hashes":{"sha1":"aaa","sha512":"bbb"},"downloads":["ftp://mirror.example/good.jar","https://cdn.example/good.jar"],"fileSize":10},
      {"path":"mods/no-url.jar","hashes":{"sha1":"ccc"},"downloads":[]},
      {"path":"mods/server-only.jar","env":{"client":"unsupported","server":"required"},"hashes":{"sha1":"ddd"},"downloads":["https://cdn.example/server-only.jar"]},
      {"path":"../escape.jar","hashes":{"sha1":"eee"},"downloads":["https://cdn.example/escape.jar"]},
      {"path":"resourcepacks/optional.zip","env":{"client":"optional","server":"unsupported"},"downloads":["https://cdn.example/optional.zip"]}
    ]"#;
    let archive = TestArchive::new(vec![(
      "modrinth.index.json",
      modrinth_manifest(files, r#"{"minecraft":"1.20.1","forge":"47.4.10"}"#).into_bytes(),
    )]);

    let manifest = ModrinthManifest::from_archive(&archive.file).unwrap();
    let instance_path = Path::new("instance-root");
    let params = manifest.download_params_from_files(instance_path);
    assert_eq!(params.len(), 2, "only good.jar and optional.zip are usable");

    let downloads: Vec<&crate::tasks::download::DownloadParam> = params
      .iter()
      .map(|param| match param {
        PTaskParam::Download(download) => download,
      })
      .collect();

    assert_eq!(downloads[0].src.as_str(), "https://cdn.example/good.jar");
    assert_eq!(downloads[0].sha1.as_deref(), Some("aaa"));
    assert_eq!(downloads[0].dest, instance_path.join("mods/good.jar"));

    assert_eq!(
      downloads[1].src.as_str(),
      "https://cdn.example/optional.zip"
    );
    assert_eq!(downloads[1].sha1, None);
    assert_eq!(
      downloads[1].dest,
      instance_path.join("resourcepacks/optional.zip")
    );
  }

  #[test]
  fn malformed_known_manifest_is_a_clear_error_without_panic() {
    let archive = TestArchive::new(vec![(
      "modrinth.index.json",
      br#"{"formatVersion":1,"files":[{"path":"mods/a.jar""#.to_vec(),
    )]);

    let Err(error) = detect_manifest(&archive.file) else {
      panic!("malformed manifest should not be recognized");
    };
    assert!(
      error.0.contains("MODPACK_MANIFEST_PARSE_ERROR"),
      "unexpected error: {}",
      error.0
    );
  }

  #[test]
  fn unknown_archive_format_is_a_clear_error() {
    let archive = TestArchive::new(vec![
      ("readme.txt", b"not a modpack".to_vec()),
      ("mods/example.jar", b"jar".to_vec()),
    ]);

    let Err(error) = detect_manifest(&archive.file) else {
      panic!("unknown format should not be recognized");
    };
    assert!(
      error.0.contains("MODPACK_MANIFEST_PARSE_ERROR"),
      "unexpected error: {}",
      error.0
    );
  }

  #[test]
  fn corrupt_zip_is_a_clear_error_without_panic() {
    let path = unique_path("corrupt", "zip");
    std::fs::write(&path, b"this is definitely not a zip archive").unwrap();
    let file = File::open(&path).unwrap();

    let Err(error) = detect_manifest(&file) else {
      panic!("corrupt zip should not be recognized");
    };
    assert!(
      error.0.contains("ZIP_FILE_PROCESS_FAILED"),
      "unexpected error: {}",
      error.0
    );
    drop(file);
    let _ = std::fs::remove_file(path);
  }

  #[test]
  fn curseforge_manifest_still_parses_with_defaulted_optional_fields() {
    let manifest = r#"{
      "name": "CF Regression Pack",
      "version": "2.0.0",
      "minecraft": {
        "version": "1.19.2",
        "modLoaders": [{"id": "forge-43.2.0", "primary": true}]
      },
      "files": []
    }"#;
    let archive = TestArchive::new(vec![("manifest.json", manifest.as_bytes().to_vec())]);

    let parsed = CurseForgeManifest::from_archive(&archive.file).unwrap();
    assert_eq!(parsed.name, "CF Regression Pack");
    assert_eq!(parsed.author, "");
    assert_eq!(parsed.get_overrides_path(), "overrides/");
    assert_eq!(parsed.get_client_version().unwrap(), "1.19.2");
    assert_eq!(
      parsed.get_mod_loader_type_version().unwrap(),
      (ModLoaderType::Forge, "43.2.0".to_string())
    );
  }

  #[test]
  fn curseforge_manifest_in_nested_folder_parses() {
    let manifest = r#"{
      "name": "Nested CF",
      "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "fabric-0.15.0", "primary": true}]},
      "overrides": "overrides",
      "files": []
    }"#;
    let archive = TestArchive::new(vec![(
      "Nested Pack/manifest.json",
      manifest.as_bytes().to_vec(),
    )]);

    let parsed = CurseForgeManifest::from_archive(&archive.file).unwrap();
    assert_eq!(parsed.get_overrides_path(), "Nested Pack/overrides/");
    assert_eq!(
      parsed.get_mod_loader_type_version().unwrap(),
      (ModLoaderType::Fabric, "0.15.0".to_string())
    );
  }

  #[test]
  fn multimc_manifest_still_parses_without_instance_cfg() {
    let manifest = r#"{
      "formatVersion": 1,
      "components": [
        {"uid": "net.minecraft", "version": "1.20.1"},
        {"uid": "net.fabricmc.fabric-loader", "version": "0.15.0"}
      ]
    }"#;
    let archive = TestArchive::new(vec![
      ("Pack/mmc-pack.json", manifest.as_bytes().to_vec()),
      ("Pack/minecraft/config/test.toml", b"value".to_vec()),
    ]);

    let parsed = MultiMcManifest::from_archive(&archive.file).unwrap();
    assert_eq!(parsed.get_client_version().unwrap(), "1.20.1");
    assert_eq!(
      parsed.get_mod_loader_type_version().unwrap(),
      (ModLoaderType::Fabric, "0.15.0".to_string())
    );
    assert_eq!(parsed.get_overrides_path(), "Pack/minecraft/");
    assert!(parsed.cfg.is_empty());
  }

  #[test]
  fn multimc_instance_cfg_name_is_preserved() {
    let manifest = r#"{
      "formatVersion": 1,
      "components": [
        {"uid": "net.minecraft", "version": "1.18.2"},
        {"uid": "net.minecraftforge", "version": "40.2.0"}
      ]
    }"#;
    let cfg = "InstanceType=OneSix\nname=Living Story\n";
    let archive = TestArchive::new(vec![
      ("mmc-pack.json", manifest.as_bytes().to_vec()),
      ("instance.cfg", cfg.as_bytes().to_vec()),
      (".minecraft/config/x.txt", b"x".to_vec()),
    ]);

    let parsed = MultiMcManifest::from_archive(&archive.file).unwrap();
    assert_eq!(
      parsed.cfg.get("name").map(String::as_str),
      Some("Living Story")
    );
    assert_eq!(parsed.get_overrides_path(), ".minecraft/");
    assert_eq!(
      parsed.get_mod_loader_type_version().unwrap(),
      (ModLoaderType::Forge, "40.2.0".to_string())
    );
  }

  #[test]
  fn safe_relative_path_rejects_escapes() {
    assert!(safe_relative_path("mods/a.jar").is_some());
    assert!(safe_relative_path("resourcepacks/sub/a.zip").is_some());
    assert!(safe_relative_path("../a.jar").is_none());
    assert!(safe_relative_path("mods/../../a.jar").is_none());
    assert!(safe_relative_path("/etc/passwd").is_none());
    assert!(safe_relative_path("C:\\Windows\\a.jar").is_none());
    assert!(safe_relative_path("").is_none());
  }

  #[test]
  #[ignore = "requires SJMCL_MODPACK_PROBE to point at an external modpack archive"]
  fn probe_external_modpack() {
    let Ok(path) = std::env::var("SJMCL_MODPACK_PROBE") else {
      return;
    };
    let file = File::open(path).unwrap();
    let manifest = detect_manifest(&file).unwrap();
    println!(
      "overrides={:?} extra={:?} mc={:?} loader={:?}",
      manifest.get_overrides_path(),
      manifest.get_extra_overrides_paths(),
      manifest.get_client_version(),
      manifest.get_mod_loader_type_version()
    );

    if let Ok(modrinth) = ModrinthManifest::from_archive(&file) {
      println!(
        "modrinth name={:?} version={:?} summary={:?} files={}",
        modrinth.name,
        modrinth.version_id,
        modrinth.summary,
        modrinth.files.len()
      );
      println!("meta={:?}", modrinth.to_meta_info());
      let params = modrinth.download_params_from_files(Path::new("instance"));
      println!("download_params={}", params.len());
    }

    if std::env::var("SJMCL_MODPACK_PROBE_EXTRACT").is_ok() {
      let dir = make_temp_dir("probe");
      let start = std::time::Instant::now();
      extract_overrides(&file, &dir.0).unwrap();
      let count = walkdir::WalkDir::new(&dir.0)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .count();
      println!("extracted_files={count} elapsed={:?}", start.elapsed());
    }
  }
}
