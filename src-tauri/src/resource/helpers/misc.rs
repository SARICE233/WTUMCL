use sjmcl_types::error::{SJMCLError, SJMCLResult};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use strum::IntoEnumIterator;
use url::Url;

use crate::launcher_config::models::{DownloadMirrorKind, LauncherConfig};
use crate::resource::models::{
  ModUpdateQuery, OtherResourceInfo, OtherResourceVersionPack, ResourceError, ResourceType,
  SourceType,
};
use crate::utils::string::contains_chinese;

// BaseUrl and path scheme of the custom download mirror, kept process-wide
// because `get_download_api` has no access to the launcher config and is called
// from many download flows with only a `SourceType`. It is synchronized by
// `get_source_priority_list` (always called before a download starts) and can also
// be set explicitly with `set_custom_mirror_target`. An empty value means "not
// configured": `get_download_api(SourceType::CustomMirror, ..)` then fails safely.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomMirrorTarget {
  pub base_url: String,
  pub kind: DownloadMirrorKind,
}

static CUSTOM_MIRROR: OnceLock<RwLock<CustomMirrorTarget>> = OnceLock::new();

fn custom_mirror_cell() -> &'static RwLock<CustomMirrorTarget> {
  CUSTOM_MIRROR.get_or_init(|| RwLock::new(CustomMirrorTarget::default()))
}

pub fn set_custom_mirror_target(base_url: &str, kind: DownloadMirrorKind) {
  // Store only a validated BaseUrl: an invalid user input falls back to "not
  // configured" so `get_download_api(SourceType::CustomMirror, ..)` errors out
  // instead of building a malformed download URL.
  let target = match normalize_custom_mirror_base(base_url) {
    Ok(base_url) => CustomMirrorTarget { base_url, kind },
    Err(_) => CustomMirrorTarget::default(),
  };
  if let Ok(mut cell) = custom_mirror_cell().write() {
    *cell = target;
  }
}

pub fn get_custom_mirror_target() -> CustomMirrorTarget {
  custom_mirror_cell()
    .read()
    .map(|target| target.clone())
    .unwrap_or_default()
}

fn normalize_custom_mirror_base_text(url: &str) -> String {
  url.trim().trim_end_matches('/').to_string()
}

/// The mirror the `CustomMirror` source refers to.
///
/// The ordered mirror list supersedes the single legacy BaseUrl: the first
/// enabled entry with a valid BaseUrl wins and its path scheme is kept, so a
/// flat (BMCLAPI-style) mirror can actually serve as the primary source. The
/// legacy BaseUrl only decides when the list has no usable entry.
fn resolve_custom_mirror_target(launcher_config: &LauncherConfig) -> CustomMirrorTarget {
  let listed = launcher_config
    .download
    .source
    .mirrors
    .iter()
    .filter(|mirror| mirror.enabled)
    .find_map(|mirror| {
      normalize_custom_mirror_base(&mirror.url)
        .ok()
        .map(|base_url| CustomMirrorTarget {
          base_url,
          kind: mirror.kind,
        })
    });

  listed
    .or_else(|| {
      normalize_custom_mirror_base(&launcher_config.download.source.custom_mirror_url)
        .ok()
        .map(|base_url| CustomMirrorTarget {
          base_url,
          kind: DownloadMirrorKind::PclPrefix,
        })
    })
    .unwrap_or_default()
}

pub fn get_source_priority_list(launcher_config: &LauncherConfig) -> Vec<SourceType> {
  let strategy = launcher_config.download.source.strategy.as_str();
  // Empty, blank and malformed BaseUrls are all treated as "not configured";
  // they must never enter the priority list.
  let custom_mirror = resolve_custom_mirror_target(launcher_config);
  let has_configured_mirror = !custom_mirror.base_url.is_empty();

  // Keep the process-wide mirror in sync so that `get_download_api` can resolve
  // `SourceType::CustomMirror` without changing its signature.
  set_custom_mirror_target(&custom_mirror.base_url, custom_mirror.kind);

  let auto_priority = match launcher_config.basic_info.is_china_mainland_ip {
    true => vec![SourceType::BMCLAPIMirror, SourceType::Official],
    false => vec![SourceType::Official, SourceType::BMCLAPIMirror],
  };

  match strategy {
    "official" => vec![SourceType::Official, SourceType::BMCLAPIMirror],
    "mirror" => vec![SourceType::BMCLAPIMirror, SourceType::Official],
    // The custom mirror is opt-in: without a BaseUrl it must never enter the
    // priority list, and "custom" behaves exactly like "auto" instead.
    "custom" => {
      if !has_configured_mirror {
        auto_priority
      } else {
        vec![
          SourceType::CustomMirror,
          SourceType::BMCLAPIMirror,
          SourceType::Official,
        ]
      }
    }
    "auto" => auto_priority,
    _ => vec![SourceType::BMCLAPIMirror, SourceType::Official],
  }
}

// https://bmclapidoc.bangbang93.com/
pub fn get_download_api(source: SourceType, resource_type: ResourceType) -> SJMCLResult<Url> {
  match source {
    SourceType::Official => get_official_download_api(resource_type),
    SourceType::BMCLAPIMirror => get_bmclapi_download_api(resource_type),
    // Reads the mirror synchronized from the launcher config; when it is empty or
    // invalid this returns `NoDownloadApi` so that callers fall back safely.
    SourceType::CustomMirror => {
      let target = get_custom_mirror_target();
      match target.kind {
        DownloadMirrorKind::PclPrefix => {
          get_custom_mirror_download_api(&target.base_url, resource_type)
        }
        DownloadMirrorKind::BmclapiFlat => {
          get_flat_mirror_download_api(&target.base_url, resource_type)
        }
      }
    }
  }
}

fn get_official_download_api(resource_type: ResourceType) -> SJMCLResult<Url> {
  match resource_type {
    ResourceType::VersionManifest => Ok(Url::parse(
      "https://launchermeta.mojang.com/mc/game/version_manifest.json",
    )?),
    ResourceType::VersionManifestV2 => Ok(Url::parse(
      "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
    )?),
    ResourceType::LauncherMeta => Ok(Url::parse("https://launchermeta.mojang.com/")?),
    ResourceType::PistonMeta => Ok(Url::parse("https://piston-meta.mojang.com/")?),
    ResourceType::PistonData => Ok(Url::parse("https://piston-data.mojang.com/")?),
    ResourceType::Launcher => Ok(Url::parse("https://launcher.mojang.com/")?),
    ResourceType::Assets => Ok(Url::parse("https://resources.download.minecraft.net/")?),
    ResourceType::Libraries => Ok(Url::parse("https://libraries.minecraft.net/")?),
    ResourceType::MojangJava => Ok(Url::parse(
      "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
    )?),
    ResourceType::ForgeMaven => Ok(Url::parse("https://files.minecraftforge.net/maven/")?),
    ResourceType::ForgeMavenNew => Ok(Url::parse("https://maven.minecraftforge.net")?),
    ResourceType::ForgeInstall => Ok(Url::parse(
      "https://maven.minecraftforge.net/net/minecraftforge/forge/",
    )?),
    ResourceType::ForgeMeta => Err(ResourceError::NoDownloadApi.into()), // https://github.com/HMCL-dev/HMCL/pull/3259/files
    ResourceType::Liteloader => Ok(Url::parse(
      "https://dl.liteloader.com/versions/versions.json",
    )?),
    ResourceType::OptiFine => Err(ResourceError::NoDownloadApi.into()), //
    ResourceType::AuthlibInjector => Ok(Url::parse("https://authlib-injector.yushi.moe/")?),
    ResourceType::FabricMeta => Ok(Url::parse("https://meta.fabricmc.net/")?),
    ResourceType::FabricMaven => Ok(Url::parse("https://maven.fabricmc.net/")?),
    // https://github.com/HMCL-dev/HMCL/blob/efd088e014bf1c113f7b3fdf73fb983087ae3f5e/HMCLCore/src/main/java/org/jackhuang/hmcl/download/neoforge/NeoForgeOfficialVersionList.java#L28
    ResourceType::NeoforgeMetaForge => Ok(Url::parse(
      "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge/",
    )?),
    ResourceType::NeoforgeMetaNeoforge => Ok(Url::parse(
      "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge/",
    )?),
    ResourceType::NeoforgeMaven | ResourceType::NeoforgeInstall => {
      Ok(Url::parse("https://maven.neoforged.net/releases/")?)
    }
    ResourceType::QuiltMaven => Ok(Url::parse("https://maven.quiltmc.org/repository/release/")?),
    ResourceType::QuiltMeta => Ok(Url::parse("https://meta.quiltmc.org/")?),
    ResourceType::CleanroomInstall => Ok(Url::parse(
      "https://hmcl.glavo.site/metadata/cleanroom/files/",
    )?),
    ResourceType::CleanroomMaven => Ok(Url::parse("https://maven.cleanroommc.com/")?),
    ResourceType::CleanroomMeta => Ok(Url::parse(
      "https://hmcl.glavo.site/metadata/cleanroom/index.json",
    )?),
  }
}

fn get_bmclapi_download_api(resource_type: ResourceType) -> SJMCLResult<Url> {
  match resource_type {
    ResourceType::VersionManifest => Ok(Url::parse(
      "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json",
    )?),
    ResourceType::VersionManifestV2 => Ok(Url::parse(
      "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json",
    )?),
    ResourceType::LauncherMeta => Ok(Url::parse("https://bmclapi2.bangbang93.com/")?),
    // BMCLAPI does not document these endpoints; report "no API" so callers keep
    // the official URL instead of requesting a possibly non-existent path.
    ResourceType::PistonMeta | ResourceType::PistonData => Err(ResourceError::NoDownloadApi.into()),
    ResourceType::Launcher => Ok(Url::parse("https://bmclapi2.bangbang93.com/")?),
    ResourceType::Assets => Ok(Url::parse("https://bmclapi2.bangbang93.com/assets/")?),
    ResourceType::Libraries => Ok(Url::parse("https://bmclapi2.bangbang93.com/maven/")?),
    ResourceType::MojangJava => Ok(Url::parse(
      "https://bmclapi2.bangbang93.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
    )?),
    ResourceType::ForgeMaven | ResourceType::ForgeMavenNew | ResourceType::NeoforgeMaven => {
      Ok(Url::parse("https://bmclapi2.bangbang93.com/maven/")?)
    }
    ResourceType::ForgeInstall => Ok(Url::parse(
      "https://bmclapi2.bangbang93.com/forge/download/",
    )?),
    ResourceType::ForgeMeta => Ok(Url::parse("https://bmclapi2.bangbang93.com/forge/")?),
    ResourceType::Liteloader => Ok(Url::parse(
      "https://bmclapi.bangbang93.com/maven/com/mumfrey/liteloader/versions.json",
    )?),
    ResourceType::AuthlibInjector => Ok(Url::parse(
      "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/",
    )?),
    ResourceType::FabricMeta => Ok(Url::parse("https://bmclapi2.bangbang93.com/fabric-meta/")?),
    ResourceType::FabricMaven => Ok(Url::parse("https://bmclapi2.bangbang93.com/maven/")?),
    ResourceType::NeoforgeMetaForge | ResourceType::NeoforgeMetaNeoforge => {
      Ok(Url::parse("https://bmclapi2.bangbang93.com/neoforge/")?)
    }
    ResourceType::NeoforgeInstall => Ok(Url::parse(
      "https://bmclapi2.bangbang93.com/neoforge/version/",
    )?),
    ResourceType::OptiFine => Ok(Url::parse("https://bmclapi2.bangbang93.com/optifine/")?),
    ResourceType::QuiltMaven => Ok(Url::parse("https://bmclapi2.bangbang93.com/maven/")?),
    ResourceType::QuiltMeta => Ok(Url::parse("https://bmclapi2.bangbang93.com/quilt-meta/")?), // seems 'not found'
    ResourceType::CleanroomInstall => Ok(Url::parse(
      "https://alist.8mi.tech/d/mirror/HMCL-Metadata/Auto/cleanroom/files/",
    )?),
    ResourceType::CleanroomMaven => Ok(Url::parse("https://maven.cleanroommc.com/")?),
    ResourceType::CleanroomMeta => Ok(Url::parse(
      "https://alist.8mi.tech/d/mirror/HMCL-Metadata/Auto/cleanroom/index.json",
    )?),
  }
}

// Resource types served by a PCL-style custom mirror, mapped from the official
// URL prefix to the mirror prefix, e.g.
// `https://piston-meta.mojang.com/` -> `{BaseUrl}/piston-meta/`.
//
// Resource types the mirror does not serve (Liteloader, OptiFine, Cleanroom,
// Quilt, AuthlibInjector, ...) deliberately reuse the BMCLAPI endpoint instead of
// building a half-formed URL from the BaseUrl.
pub fn get_custom_mirror_download_api(
  base_url: &str,
  resource_type: ResourceType,
) -> SJMCLResult<Url> {
  let base_url = normalize_custom_mirror_base(base_url)?;

  let path = match resource_type {
    ResourceType::LauncherMeta => "launchermeta/",
    ResourceType::VersionManifest => "launchermeta/mc/game/version_manifest.json",
    ResourceType::VersionManifestV2 => "launchermeta/mc/game/version_manifest_v2.json",
    ResourceType::PistonMeta => "piston-meta/",
    ResourceType::PistonData => "piston-data/",
    ResourceType::Launcher => "launcher/",
    ResourceType::Assets => "resources/",
    ResourceType::Libraries => "libraries/",
    ResourceType::ForgeMaven => "forge-files/maven/",
    ResourceType::ForgeMavenNew => "forge-maven/",
    ResourceType::NeoforgeMaven => "neoforge-maven/",
    ResourceType::FabricMaven => "fabric-maven/",
    _ => return get_bmclapi_download_api(resource_type),
  };

  Ok(Url::parse(&format!("{}/{}", base_url, path))?)
}

// Resource types served by a BMCLAPI-style (flat) custom mirror, mapped to the
// full URL the download flow requests:
// ` {BaseUrl}/mc/game/version_manifest.json`, `{BaseUrl}/maven/`, `{BaseUrl}/assets/`, ...
//
// A flat mirror exposes its own version endpoints instead of mirroring the
// piston hosts path-for-path, so PistonMeta/PistonData deliberately resolve to
// nothing: the version info keeps pointing at Mojang and the mirror only serves
// the objects it advertises.
pub fn get_flat_mirror_download_api(
  base_url: &str,
  resource_type: ResourceType,
) -> SJMCLResult<Url> {
  let base_url = normalize_custom_mirror_base(base_url)?;
  let base = base_url.as_str();

  let url = match resource_type {
    ResourceType::VersionManifest => format!("{base}/mc/game/version_manifest.json"),
    ResourceType::VersionManifestV2 => format!("{base}/mc/game/version_manifest_v2.json"),
    ResourceType::LauncherMeta | ResourceType::Launcher => format!("{base}/"),
    ResourceType::Assets => format!("{base}/assets/"),
    ResourceType::Libraries
    | ResourceType::ForgeMaven
    | ResourceType::ForgeMavenNew
    | ResourceType::NeoforgeMaven
    | ResourceType::FabricMaven
    | ResourceType::QuiltMaven
    | ResourceType::Liteloader => format!("{base}/maven/"),
    ResourceType::ForgeMeta => format!("{base}/forge/"),
    ResourceType::ForgeInstall => format!("{base}/forge/download/"),
    ResourceType::FabricMeta => format!("{base}/fabric-meta/"),
    ResourceType::NeoforgeMetaForge | ResourceType::NeoforgeMetaNeoforge => {
      format!("{base}/neoforge/")
    }
    ResourceType::NeoforgeInstall => format!("{base}/neoforge/version/"),
    ResourceType::OptiFine => format!("{base}/optifine/"),
    ResourceType::QuiltMeta => format!("{base}/quilt-meta/"),
    ResourceType::AuthlibInjector => format!("{base}/mirrors/authlib-injector/"),
    ResourceType::MojangJava => {
      format!("{base}/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json")
    }
    ResourceType::PistonMeta
    | ResourceType::PistonData
    | ResourceType::CleanroomMaven
    | ResourceType::CleanroomMeta
    | ResourceType::CleanroomInstall => return Err(ResourceError::NoDownloadApi.into()),
  };

  Ok(Url::parse(&url)?)
}

// Validate and normalize a user-provided custom mirror BaseUrl.
// Empty, non-http(s), host-less or query/fragment-carrying values are rejected
// with `NoDownloadApi` instead of producing a malformed download URL.
fn normalize_custom_mirror_base(base_url: &str) -> SJMCLResult<String> {
  let base_url = normalize_custom_mirror_base_text(base_url);
  if base_url.is_empty() {
    return Err(ResourceError::NoDownloadApi.into());
  }

  let parsed = Url::parse(&base_url)?;
  if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
    return Err(ResourceError::NoDownloadApi.into());
  }
  if parsed.query().is_some() || parsed.fragment().is_some() {
    return Err(ResourceError::NoDownloadApi.into());
  }

  Ok(base_url)
}

#[expect(dead_code, reason = "reserved for future use")]
pub fn convert_url_source_type(
  url: &Url,
  resource_type: &ResourceType,
  src_type: &SourceType,
  dst_type: &SourceType,
) -> SJMCLResult<Url> {
  let url_str = url.as_str();
  let src_api = get_download_api(*src_type, *resource_type)?;
  let dst_api = get_download_api(*dst_type, *resource_type)?;
  if url_str.starts_with(src_api.as_str()) {
    Ok(Url::parse(
      url_str
        .replacen(src_api.as_str(), dst_api.as_str(), 1)
        .as_str(),
    )?)
  } else {
    Err(ResourceError::NoDownloadApi.into())
  }
}

pub fn convert_url_to_target_source(
  url: &Url,
  resource_types: &[ResourceType],
  dst_type: &SourceType,
) -> SJMCLResult<Url> {
  let url_str = url.as_str();
  let resource_candidates = if resource_types.is_empty() {
    ResourceType::iter().collect::<Vec<_>>()
  } else {
    resource_types.to_vec()
  };

  for resource_type in resource_candidates {
    let dst_api = match get_download_api(*dst_type, resource_type) {
      Ok(api) => api,
      Err(_) => return Ok(url.clone()), // If destination API is not available, return the original URL
    };

    for src_type in SourceType::iter() {
      if &src_type == dst_type {
        continue;
      }

      if let Ok(src_api) = get_download_api(src_type, resource_type)
        && url_str.starts_with(src_api.as_str())
      {
        let new_url_str = url_str.replacen(src_api.as_str(), dst_api.as_str(), 1);
        return Ok(Url::parse(&new_url_str)?);
      }
    }
  }

  // If no replacement occurred, return the original URL
  Ok(url.clone())
}

/// Rewrite a Mojang game-file URL to the custom mirror only when it is the
/// primary source. Other strategies and invalid / unrecognized URLs are
/// returned untouched, so a broken custom mirror never produces a bad download
/// URL by itself.
pub fn convert_url_to_primary_source(
  url: &Url,
  resource_types: &[ResourceType],
  priority_list: &[SourceType],
) -> Url {
  let Some(dst_type) = priority_list.first() else {
    return url.clone();
  };
  if *dst_type != SourceType::CustomMirror {
    return url.clone();
  }
  convert_url_to_target_source(url, resource_types, dst_type).unwrap_or_else(|_| url.clone())
}

pub fn version_pack_sort(a: &OtherResourceVersionPack, b: &OtherResourceVersionPack) -> Ordering {
  fn parse_version(version: &str) -> (Vec<u32>, String) {
    let mut version_numbers = Vec::new();
    let mut suffix = String::new();

    for part in version.split('.') {
      if let Some(dash_pos) = part.find('-') {
        let (num_part, suffix_part) = part.split_at(dash_pos);
        if let Ok(num) = num_part.parse::<u32>() {
          version_numbers.push(num);
          suffix = suffix_part.to_string();
        }
        break;
      } else if let Ok(num) = part.parse::<u32>() {
        version_numbers.push(num);
      }
    }

    (version_numbers, suffix)
  }

  fn compare_versions_with_suffix(
    v1: &[u32],
    suffix1: &str,
    v2: &[u32],
    suffix2: &str,
  ) -> Ordering {
    for (a, b) in v1.iter().zip(v2.iter()) {
      match a.cmp(b) {
        Ordering::Equal => continue,
        other => return other,
      }
    }

    match v1.len().cmp(&v2.len()) {
      Ordering::Equal => match (suffix1.is_empty(), suffix2.is_empty()) {
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => suffix1.cmp(suffix2),
      },
      other => other,
    }
  }

  let (version_a, suffix_a) = parse_version(&a.name);
  let (version_b, suffix_b) = parse_version(&b.name);

  compare_versions_with_suffix(&version_a, &suffix_a, &version_b, &suffix_b).reverse()
}

pub(crate) fn levenshtein_distance(a: &str, b: &str) -> usize {
  let b_chars: Vec<char> = b.chars().collect();
  let mut prev: Vec<usize> = (0..=b_chars.len()).collect();

  for (i, a_ch) in a.chars().enumerate() {
    let mut current = Vec::with_capacity(b_chars.len() + 1);
    current.push(i + 1);

    for (j, b_ch) in b_chars.iter().enumerate() {
      let cost = if a_ch == *b_ch { 0 } else { 1 };
      let insertion = current[j] + 1;
      let deletion = prev[j + 1] + 1;
      let substitution = prev[j] + cost;
      current.push(insertion.min(deletion).min(substitution));
    }

    prev = current;
  }

  *prev.last().unwrap_or(&0)
}

pub fn sort_localized_search_results(list: &mut Vec<OtherResourceInfo>, search_query: &str) {
  const CONTAIN_CHINESE_WEIGHT: i64 = 10;

  let search_query = search_query
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");

  if search_query.is_empty() {
    return;
  }

  let mut translated_results = Vec::new();
  let mut untranslated_results = Vec::new();

  for resource in list.drain(..) {
    let contains_chinese_title = resource
      .translated_name
      .as_deref()
      .map_or_else(|| contains_chinese(&resource.name), contains_chinese);

    if contains_chinese_title {
      translated_results.push(resource);
    } else {
      untranslated_results.push(resource);
    }
  }

  translated_results.sort_by_key(|resource| {
    let display_name = resource
      .translated_name
      .as_deref()
      .unwrap_or(resource.name.as_str());

    let mut diff = levenshtein_distance(&search_query, display_name) as i64;
    for ch in search_query.chars() {
      if display_name.contains(ch) {
        diff -= CONTAIN_CHINESE_WEIGHT;
      }
    }

    diff
  });

  list.extend(translated_results);
  list.extend(untranslated_results);
}

pub fn resolve_mod_update_paths(
  mods_dir: &Path,
  queries: &[ModUpdateQuery],
) -> SJMCLResult<Vec<(PathBuf, PathBuf)>> {
  let canonical_mods_dir = mods_dir.canonicalize().map_err(|error| {
    SJMCLError(format!(
      "Failed to resolve mods directory {}: {}",
      mods_dir.display(),
      error
    ))
  })?;
  let mut targets = HashSet::new();
  let mut paths = Vec::with_capacity(queries.len());

  for query in queries {
    let old_file_path = PathBuf::from(&query.old_file_path);
    if !old_file_path.is_file() {
      return Err(SJMCLError(format!(
        "Old mod file does not exist: {}",
        old_file_path.display()
      )));
    }

    let old_parent = old_file_path.parent().ok_or_else(|| {
      SJMCLError(format!(
        "Old mod file has no parent directory: {}",
        old_file_path.display()
      ))
    })?;
    let canonical_old_parent = old_parent.canonicalize().map_err(|error| {
      SJMCLError(format!(
        "Failed to resolve old mod directory {}: {}",
        old_parent.display(),
        error
      ))
    })?;
    if !canonical_old_parent.starts_with(&canonical_mods_dir) {
      return Err(SJMCLError(format!(
        "Old mod file is outside the instance mods directory: {}",
        old_file_path.display()
      )));
    }

    let new_file_name = Path::new(&query.file_name);
    if new_file_name.file_name() != Some(new_file_name.as_os_str()) {
      return Err(SJMCLError(format!(
        "Invalid mod update file name: {}",
        query.file_name
      )));
    }

    let new_file_path = old_parent.join(new_file_name);
    let canonical_target = canonical_old_parent.join(new_file_name);
    if !targets.insert(canonical_target) {
      return Err(SJMCLError(format!(
        "Duplicate mod update target: {}",
        new_file_path.display()
      )));
    }
    if new_file_path != old_file_path && new_file_path.exists() {
      return Err(SJMCLError(format!(
        "Mod update target already exists: {}",
        new_file_path.display()
      )));
    }

    paths.push((old_file_path, new_file_path));
  }

  Ok(paths)
}

#[cfg(test)]
mod download_source_tests {
  use super::*;
  use std::sync::{Mutex, MutexGuard};

  use crate::launcher_config::models::DownloadMirrorConfig;

  const CUSTOM_MIRROR: &str = "https://mcmirror.bvvd.mom";

  // The custom mirror BaseUrl lives in a process-wide cell, so tests touching it
  // must be serialized.
  static BASE_URL_LOCK: Mutex<()> = Mutex::new(());

  // The legacy single BaseUrl is the PclPrefix flavour of the same cell; tests
  // that predate the mirror list keep configuring it this way.
  fn set_legacy_mirror(url: &str) {
    set_custom_mirror_target(url, DownloadMirrorKind::PclPrefix);
  }

  fn legacy_mirror_url() -> String {
    get_custom_mirror_target().base_url
  }

  fn lock_base_url() -> MutexGuard<'static, ()> {
    let guard = BASE_URL_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    set_legacy_mirror("");
    guard
  }

  fn test_config(
    strategy: &str,
    custom_mirror_url: &str,
    is_china_mainland_ip: bool,
  ) -> LauncherConfig {
    let mut config = LauncherConfig::default();
    config.download.source.strategy = strategy.to_string();
    config.download.source.custom_mirror_url = custom_mirror_url.to_string();
    config.basic_info.is_china_mainland_ip = is_china_mainland_ip;
    config
  }

  fn api_str(source: SourceType, resource_type: ResourceType) -> String {
    get_download_api(source, resource_type)
      .expect("download api should resolve")
      .to_string()
  }

  #[test]
  fn custom_mirror_maps_official_prefixes() {
    let _guard = lock_base_url();
    set_legacy_mirror(CUSTOM_MIRROR);

    let cases = [
      (
        ResourceType::LauncherMeta,
        "https://mcmirror.bvvd.mom/launchermeta/",
      ),
      (
        ResourceType::PistonMeta,
        "https://mcmirror.bvvd.mom/piston-meta/",
      ),
      (
        ResourceType::PistonData,
        "https://mcmirror.bvvd.mom/piston-data/",
      ),
      (ResourceType::Assets, "https://mcmirror.bvvd.mom/resources/"),
      (
        ResourceType::Libraries,
        "https://mcmirror.bvvd.mom/libraries/",
      ),
      (
        ResourceType::FabricMaven,
        "https://mcmirror.bvvd.mom/fabric-maven/",
      ),
      (
        ResourceType::Launcher,
        "https://mcmirror.bvvd.mom/launcher/",
      ),
      (
        ResourceType::ForgeMaven,
        "https://mcmirror.bvvd.mom/forge-files/maven/",
      ),
      (
        ResourceType::ForgeMavenNew,
        "https://mcmirror.bvvd.mom/forge-maven/",
      ),
      (
        ResourceType::NeoforgeMaven,
        "https://mcmirror.bvvd.mom/neoforge-maven/",
      ),
    ];

    for (resource_type, expected) in cases {
      assert_eq!(
        api_str(SourceType::CustomMirror, resource_type),
        expected,
        "unexpected custom mirror api for {resource_type:?}"
      );
    }
  }

  #[test]
  fn custom_mirror_url_comes_from_launcher_config() {
    let _guard = lock_base_url();
    let config = test_config("custom", CUSTOM_MIRROR, false);

    assert_eq!(
      get_source_priority_list(&config).first(),
      Some(&SourceType::CustomMirror)
    );
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::Libraries),
      "https://mcmirror.bvvd.mom/libraries/"
    );
  }

  #[test]
  fn custom_mirror_trailing_slash_does_not_produce_double_slash() {
    let _guard = lock_base_url();

    let config = test_config("custom", "https://mcmirror.bvvd.mom/", false);
    get_source_priority_list(&config);

    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::LauncherMeta),
      "https://mcmirror.bvvd.mom/launchermeta/"
    );
    assert!(
      !api_str(SourceType::CustomMirror, ResourceType::LauncherMeta)
        .contains("mcmirror.bvvd.mom//")
    );
  }

  #[test]
  fn custom_mirror_keeps_base_url_path_prefix() {
    let _guard = lock_base_url();
    set_legacy_mirror("https://example.com/mirror/");

    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::PistonData),
      "https://example.com/mirror/piston-data/"
    );
  }

  #[test]
  fn empty_custom_mirror_url_never_joins_priority_list() {
    let _guard = lock_base_url();

    for is_china_mainland_ip in [true, false] {
      let custom = test_config("custom", "", is_china_mainland_ip);
      let auto = test_config("auto", "", is_china_mainland_ip);

      let priority = get_source_priority_list(&custom);
      assert_eq!(priority, get_source_priority_list(&auto));
      assert!(!priority.contains(&SourceType::CustomMirror));

      // A whitespace-only BaseUrl counts as "not configured" as well.
      let custom = test_config("custom", "   ", is_china_mainland_ip);
      assert_eq!(
        get_source_priority_list(&custom),
        get_source_priority_list(&auto)
      );
    }
  }

  #[test]
  fn configured_custom_mirror_is_first_then_bmclapi_then_official() {
    let _guard = lock_base_url();

    let priority = get_source_priority_list(&test_config("custom", CUSTOM_MIRROR, true));
    assert_eq!(
      priority,
      vec![
        SourceType::CustomMirror,
        SourceType::BMCLAPIMirror,
        SourceType::Official
      ]
    );

    let priority = get_source_priority_list(&test_config("custom", CUSTOM_MIRROR, false));
    assert_eq!(
      priority,
      vec![
        SourceType::CustomMirror,
        SourceType::BMCLAPIMirror,
        SourceType::Official
      ]
    );
  }

  #[test]
  fn existing_strategies_are_unchanged() {
    let _guard = lock_base_url();

    assert_eq!(
      get_source_priority_list(&test_config("official", CUSTOM_MIRROR, true)),
      vec![SourceType::Official, SourceType::BMCLAPIMirror]
    );
    assert_eq!(
      get_source_priority_list(&test_config("mirror", CUSTOM_MIRROR, false)),
      vec![SourceType::BMCLAPIMirror, SourceType::Official]
    );
    assert_eq!(
      get_source_priority_list(&test_config("auto", CUSTOM_MIRROR, false)),
      vec![SourceType::Official, SourceType::BMCLAPIMirror]
    );
  }

  #[test]
  fn custom_mirror_without_base_url_fails_without_panicking() {
    let _guard = lock_base_url();

    // Empty, blank and whitespace-only BaseUrls must all be rejected explicitly.
    for base_url in ["", "   ", "///"] {
      assert!(get_custom_mirror_download_api(base_url, ResourceType::Libraries).is_err());
    }

    // Without a configured BaseUrl the source type resolves to an error, never a panic.
    assert!(get_download_api(SourceType::CustomMirror, ResourceType::LauncherMeta).is_err());
    assert!(get_download_api(SourceType::CustomMirror, ResourceType::Libraries).is_err());
  }

  #[test]
  fn invalid_custom_mirror_base_urls_are_rejected() {
    let _guard = lock_base_url();

    for base_url in [
      "mcmirror.bvvd.mom",
      "ftp://mcmirror.bvvd.mom",
      "file:///tmp/mirror",
      "https://mcmirror.bvvd.mom/?token=1",
      "https://mcmirror.bvvd.mom/#frag",
    ] {
      assert!(
        get_custom_mirror_download_api(base_url, ResourceType::LauncherMeta).is_err(),
        "base url {base_url} should be rejected"
      );
    }
  }

  #[test]
  fn unmapped_resource_types_fall_back_to_bmclapi() {
    let _guard = lock_base_url();
    set_legacy_mirror(CUSTOM_MIRROR);

    for resource_type in [
      ResourceType::Liteloader,
      ResourceType::OptiFine,
      ResourceType::AuthlibInjector,
      ResourceType::FabricMeta,
      ResourceType::CleanroomMaven,
      ResourceType::QuiltMeta,
      ResourceType::ForgeInstall,
      ResourceType::NeoforgeInstall,
      ResourceType::MojangJava,
    ] {
      let custom = get_download_api(SourceType::CustomMirror, resource_type);
      let bmcl = get_download_api(SourceType::BMCLAPIMirror, resource_type);
      match (custom, bmcl) {
        (Ok(custom), Ok(bmcl)) => {
          assert_eq!(
            custom, bmcl,
            "expected bmclapi fallback for {resource_type:?}"
          );
          assert!(!custom.as_str().contains("mcmirror.bvvd.mom"));
        }
        (Err(_), Err(_)) => {}
        (custom, bmcl) => {
          panic!("inconsistent fallback for {resource_type:?}: {custom:?} vs {bmcl:?}")
        }
      }
    }
  }

  #[test]
  fn converts_official_urls_to_custom_mirror_prefixes() {
    let _guard = lock_base_url();
    get_source_priority_list(&test_config("custom", CUSTOM_MIRROR, false));

    let url = Url::parse("https://piston-data.mojang.com/v1/objects/abc/client.jar").unwrap();
    let converted =
      convert_url_to_target_source(&url, &[ResourceType::PistonData], &SourceType::CustomMirror)
        .unwrap();
    assert_eq!(
      converted.as_str(),
      "https://mcmirror.bvvd.mom/piston-data/v1/objects/abc/client.jar"
    );

    // URLs that belong to no known source are returned untouched.
    let url = Url::parse("https://example.com/libraries/a/b/1.0/b-1.0.jar").unwrap();
    let unchanged = convert_url_to_target_source(&url, &[], &SourceType::CustomMirror).unwrap();
    assert_eq!(unchanged, url);
  }

  #[test]
  fn invalid_custom_mirror_url_never_joins_priority_list() {
    let _guard = lock_base_url();

    for base_url in [
      "mcmirror.bvvd.mom",
      "ftp://mcmirror.bvvd.mom",
      "https://mcmirror.bvvd.mom/?token=1",
      "not a url",
    ] {
      let custom = test_config("custom", base_url, false);
      let priority = get_source_priority_list(&custom);
      assert_eq!(
        priority,
        vec![SourceType::Official, SourceType::BMCLAPIMirror],
        "invalid base url {base_url} must fall back to auto priority"
      );
      assert!(!priority.contains(&SourceType::CustomMirror));
      assert_eq!(legacy_mirror_url(), "", "invalid base url must be cleared");
    }
  }

  #[test]
  fn custom_mirror_url_is_trimmed_before_use() {
    let _guard = lock_base_url();

    let priority = get_source_priority_list(&test_config(
      "custom",
      "  https://example.com/mirror/  ",
      true,
    ));
    assert_eq!(priority.first(), Some(&SourceType::CustomMirror));
    assert_eq!(legacy_mirror_url(), "https://example.com/mirror");
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::Libraries),
      "https://example.com/mirror/libraries/"
    );
  }

  #[test]
  fn primary_source_conversion_only_rewrites_for_configured_custom_mirror() {
    let _guard = lock_base_url();

    let piston_data =
      Url::parse("https://piston-data.mojang.com/v1/objects/abc/client.jar").unwrap();
    let custom = [SourceType::CustomMirror, SourceType::BMCLAPIMirror];
    let official = [SourceType::Official, SourceType::BMCLAPIMirror];

    // An unconfigured custom mirror is still an empty BaseUrl: keep the official
    // URL instead of producing an invalid download address.
    assert_eq!(
      convert_url_to_primary_source(&piston_data, &[ResourceType::PistonData], &custom),
      piston_data
    );
    // Other strategies are unaffected.
    assert_eq!(
      convert_url_to_primary_source(&piston_data, &[ResourceType::PistonData], &official),
      piston_data
    );

    set_legacy_mirror(CUSTOM_MIRROR);
    assert_eq!(
      convert_url_to_primary_source(&piston_data, &[ResourceType::PistonData], &custom).as_str(),
      "https://mcmirror.bvvd.mom/piston-data/v1/objects/abc/client.jar"
    );
  }

  #[test]
  fn installer_resource_types_fall_back_to_bmclapi_not_the_custom_base() {
    let _guard = lock_base_url();
    set_legacy_mirror(CUSTOM_MIRROR);

    for resource_type in [ResourceType::ForgeInstall, ResourceType::NeoforgeInstall] {
      let custom = get_download_api(SourceType::CustomMirror, resource_type)
        .expect("unmapped resource type should fall back to bmclapi");
      let bmcl = get_download_api(SourceType::BMCLAPIMirror, resource_type).unwrap();
      assert_eq!(custom, bmcl);
      assert!(!custom.as_str().contains("mcmirror.bvvd.mom"));
    }

    // Without a valid BaseUrl the source resolves to an error instead of an URL.
    set_legacy_mirror("not a url");
    assert_eq!(legacy_mirror_url(), "");
    assert!(get_download_api(SourceType::CustomMirror, ResourceType::ForgeInstall).is_err());
  }

  fn flat_mirror(url: &str) -> DownloadMirrorConfig {
    DownloadMirrorConfig {
      url: url.to_string(),
      kind: DownloadMirrorKind::BmclapiFlat,
      enabled: true,
    }
  }

  #[test]
  fn listed_flat_mirror_becomes_the_custom_source() {
    let _guard = lock_base_url();
    let mut config = test_config("custom", "", true);
    config.download.source.mirrors = vec![flat_mirror(CUSTOM_MIRROR)];

    let priority = get_source_priority_list(&config);
    assert_eq!(priority.first(), Some(&SourceType::CustomMirror));
    assert_eq!(legacy_mirror_url(), CUSTOM_MIRROR);
    assert_eq!(
      get_custom_mirror_target().kind,
      DownloadMirrorKind::BmclapiFlat
    );
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::VersionManifest),
      "https://mcmirror.bvvd.mom/mc/game/version_manifest.json"
    );
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::Libraries),
      "https://mcmirror.bvvd.mom/maven/"
    );
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::Assets),
      "https://mcmirror.bvvd.mom/assets/"
    );
    // A flat mirror rewrites its own version manifests; there is no path to
    // guess for the piston hosts, so the caller must keep the original URL.
    assert!(get_download_api(SourceType::CustomMirror, ResourceType::PistonMeta).is_err());
  }

  #[test]
  fn listed_mirror_kind_selects_the_path_scheme() {
    let _guard = lock_base_url();
    let mut config = test_config("custom", "", true);
    config.download.source.mirrors = vec![DownloadMirrorConfig {
      url: CUSTOM_MIRROR.to_string(),
      kind: DownloadMirrorKind::PclPrefix,
      enabled: true,
    }];

    get_source_priority_list(&config);
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::Libraries),
      "https://mcmirror.bvvd.mom/libraries/"
    );
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::VersionManifest),
      "https://mcmirror.bvvd.mom/launchermeta/mc/game/version_manifest.json"
    );
  }

  #[test]
  fn an_unusable_mirror_entry_falls_back_to_the_legacy_base_url() {
    let _guard = lock_base_url();
    let mut config = test_config("custom", CUSTOM_MIRROR, true);
    config.download.source.mirrors = vec![
      flat_mirror("not a url"),
      DownloadMirrorConfig {
        url: "https://disabled.example".to_string(),
        kind: DownloadMirrorKind::BmclapiFlat,
        enabled: false,
      },
    ];

    get_source_priority_list(&config);
    assert_eq!(legacy_mirror_url(), CUSTOM_MIRROR);
    assert_eq!(
      get_custom_mirror_target().kind,
      DownloadMirrorKind::PclPrefix
    );
    assert_eq!(
      api_str(SourceType::CustomMirror, ResourceType::Libraries),
      "https://mcmirror.bvvd.mom/libraries/"
    );
  }
}
