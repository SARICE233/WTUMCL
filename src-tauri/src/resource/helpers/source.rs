use std::collections::HashSet;

use strum::IntoEnumIterator;
use url::Url;

use crate::launcher_config::models::{DownloadMirrorConfig, DownloadMirrorKind, LauncherConfig};
use crate::resource::helpers::misc::{get_custom_mirror_download_api, get_download_api};
use crate::resource::models::{ResourceType, SourceType};

/// Official hosts whose URLs may be re-pointed at another source. A URL that
/// does not start with one of these belongs to a third-party maven (fabricmc,
/// forge, neoforged, ...) and is served by its own source entry, so it must
/// never be rewritten onto a Mojang-style path.
const OFFICIAL_PREFIXES: &[&str] = &[
  "https://piston-meta.mojang.com",
  "https://piston-data.mojang.com",
  "https://launchermeta.mojang.com",
  "https://launcher.mojang.com",
  "https://resources.download.minecraft.net",
  "https://libraries.minecraft.net",
];

/// Official Maven repositories that may serve the same coordinates for the
/// Maven-layout resource types. Which one goes first is decided per namespace
/// by `maven_priority`.
const MAVEN_LIBRARIES: &str = "https://libraries.minecraft.net";
const MAVEN_FABRIC: &str = "https://maven.fabricmc.net";
const MAVEN_FORGE: &str = "https://maven.minecraftforge.net";
const MAVEN_NEOFORGE: &str = "https://maven.neoforged.net/releases";
const MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2";

/// Download base URL of one user mirror for one resource type.
///
/// `None` means "this mirror cannot serve that resource type"; callers must then
/// keep the previous URL instead of building a path the mirror never implements.
pub fn mirror_download_api(
  mirror: &DownloadMirrorConfig,
  resource_type: ResourceType,
) -> Option<Url> {
  match mirror.kind {
    DownloadMirrorKind::PclPrefix => {
      get_custom_mirror_download_api(&mirror.url, resource_type).ok()
    }
    DownloadMirrorKind::BmclapiFlat => flat_mirror_api(&mirror.url, resource_type),
  }
}

/// BMCLAPI-style mirrors share one flat path scheme, so only the base differs.
/// Prefix-shaped entries are returned without a trailing slash: the caller
/// appends the official URL suffix it is rewriting.
fn flat_mirror_api(base: &str, resource_type: ResourceType) -> Option<Url> {
  let base = base.trim().trim_end_matches('/');
  if base.is_empty() {
    return None;
  }

  let path = match resource_type {
    // The official suffix already carries `/mc/game/version_manifest*.json`.
    ResourceType::LauncherMeta
    | ResourceType::VersionManifest
    | ResourceType::VersionManifestV2 => base.to_string(),
    ResourceType::Assets => format!("{base}/assets"),
    ResourceType::Launcher => base.to_string(),
    ResourceType::Libraries
    | ResourceType::ForgeMaven
    | ResourceType::ForgeMavenNew
    | ResourceType::NeoforgeMaven
    | ResourceType::FabricMaven
    | ResourceType::QuiltMaven
    | ResourceType::Liteloader => format!("{base}/maven"),
    ResourceType::ForgeMeta => format!("{base}/forge"),
    ResourceType::ForgeInstall => format!("{base}/forge/download"),
    ResourceType::FabricMeta => format!("{base}/fabric-meta"),
    ResourceType::NeoforgeMetaForge | ResourceType::NeoforgeMetaNeoforge => {
      format!("{base}/neoforge")
    }
    ResourceType::NeoforgeInstall => format!("{base}/neoforge/version"),
    ResourceType::OptiFine => format!("{base}/optifine"),
    ResourceType::QuiltMeta => format!("{base}/quilt-meta"),
    ResourceType::AuthlibInjector => format!("{base}/mirrors/authlib-injector"),
    ResourceType::MojangJava => {
      format!("{base}/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json")
    }
    // Flat mirrors expose these through their own version endpoints; guessing a
    // path would only produce 404s, so report nothing.
    ResourceType::PistonMeta
    | ResourceType::PistonData
    | ResourceType::CleanroomMaven
    | ResourceType::CleanroomMeta
    | ResourceType::CleanroomInstall => return None,
  };

  Url::parse(&path).ok()
}

/// Mirrors the user actually wants us to use, in their configured order.
///
/// The legacy single BaseUrl predates the list; when the list is empty it is
/// promoted to a one-element list, so configurations written by older builds
/// keep behaving exactly as before.
pub fn configured_mirrors(config: &LauncherConfig) -> Vec<DownloadMirrorConfig> {
  let configured: Vec<DownloadMirrorConfig> = config
    .download
    .source
    .mirrors
    .iter()
    .filter(|mirror| mirror.enabled && !mirror.url.trim().is_empty())
    .cloned()
    .collect();

  if !configured.is_empty() {
    return configured;
  }

  let legacy = config.download.source.custom_mirror_url.trim();
  if legacy.is_empty() {
    Vec::new()
  } else {
    vec![DownloadMirrorConfig {
      url: legacy.to_string(),
      kind: DownloadMirrorKind::PclPrefix,
      enabled: true,
    }]
  }
}

/// Source order for the current configuration.
///
/// Same rules as `misc::get_source_priority_list`, but without the global BaseUrl
/// side effect: candidate generation is a pure query and must not mutate process
/// state (that also makes the tests independent).
fn source_order(config: &LauncherConfig) -> Vec<SourceType> {
  let has_mirror = !configured_mirrors(config).is_empty();
  let auto = match config.basic_info.is_china_mainland_ip {
    true => vec![SourceType::BMCLAPIMirror, SourceType::Official],
    false => vec![SourceType::Official, SourceType::BMCLAPIMirror],
  };

  match config.download.source.strategy.as_str() {
    "official" => vec![SourceType::Official, SourceType::BMCLAPIMirror],
    "mirror" => vec![SourceType::BMCLAPIMirror, SourceType::Official],
    "custom" => {
      if has_mirror {
        vec![
          SourceType::CustomMirror,
          SourceType::BMCLAPIMirror,
          SourceType::Official,
        ]
      } else {
        auto
      }
    }
    "auto" => auto,
    _ => vec![SourceType::BMCLAPIMirror, SourceType::Official],
  }
}

/// Ordered candidate URLs for one object.
///
/// The first entry is always the URL the caller already had, so a caller that
/// only reads `candidates[0]` keeps the previous single-source behaviour.
pub fn candidates(url: &Url, resource_types: &[ResourceType], config: &LauncherConfig) -> Vec<Url> {
  let mut result: Vec<Url> = vec![url.clone()];
  let resource_types = if resource_types.is_empty() {
    ResourceType::iter().collect::<Vec<_>>()
  } else {
    resource_types.to_vec()
  };

  for source in source_order(config) {
    for resource_type in &resource_types {
      match source {
        SourceType::CustomMirror => {
          for mirror in configured_mirrors(config) {
            if let Some(api) = mirror_download_api(&mirror, *resource_type) {
              push_rewritten(&mut result, url, &api);
            }
          }
        }
        other => {
          if let Ok(api) = get_download_api(other, *resource_type) {
            push_rewritten(&mut result, url, &api);
          }
        }
      }
    }
  }

  push_reverse_sources(&mut result, url, &resource_types, config);
  dedupe(result)
}

/// Appends `api` + the official suffix of `url`, when `url` still points at a
/// Mojang host and the rewrite actually changes something.
fn push_rewritten(result: &mut Vec<Url>, url: &Url, api: &Url) {
  let api = api.as_str().trim_end_matches('/');
  if api.is_empty() {
    return;
  }

  let Some(suffix) = split_official_prefix(url.as_str()) else {
    return;
  };

  if let Ok(candidate) = Url::parse(&format!("{api}{suffix}")) {
    result.push(candidate);
  }
}

fn split_official_prefix(url: &str) -> Option<&str> {
  OFFICIAL_PREFIXES
    .iter()
    .find_map(|prefix| url.strip_prefix(prefix))
    // A host such as `libraries.minecraft.net.evil.example` shares the prefix
    // but is not a Mojang URL: only a path boundary may follow the prefix.
    .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
}

/// Appends the failover candidates hidden behind a mirror-form URL.
///
/// `push_rewritten` only understands official URLs, so a URL that already
/// points at BMCLAPI or a configured mirror would otherwise keep a single
/// candidate and every download would have exactly one source to try.
fn push_reverse_sources(
  result: &mut Vec<Url>,
  url: &Url,
  resource_types: &[ResourceType],
  config: &LauncherConfig,
) {
  let Some((base, matched)) = matched_reverse_base(url, resource_types, config) else {
    return;
  };

  // `base` was matched as a prefix of `url`, so the split sits on a char
  // boundary and the suffix is either empty or starts with '/'.
  let suffix = &url.as_str()[base.len()..];
  for resource_type in matched {
    for alternative in alternative_bases(resource_type, suffix, config) {
      if let Ok(candidate) = Url::parse(&format!("{alternative}{suffix}")) {
        result.push(candidate);
      }
    }
  }
}

/// Longest known download base `url` already points at, together with the
/// resource types that base serves.
///
/// Bases match on a path boundary (`url == base` or `url` starts with
/// `base + "/"`), so `.../mavenX/foo.jar` is not mistaken for an object of
/// `.../maven/`, and the longest base wins when several are nested.
fn matched_reverse_base(
  url: &Url,
  resource_types: &[ResourceType],
  config: &LauncherConfig,
) -> Option<(String, Vec<ResourceType>)> {
  let text = url.as_str();
  let mut best: Option<String> = None;
  let mut matched: Vec<ResourceType> = Vec::new();

  for resource_type in resource_types {
    for api in derivation_bases(*resource_type, config) {
      // A bare host base cannot tell which kind of object a URL below it is,
      // so it must never be used for reverse derivation: otherwise a typo such
      // as `.../mavenX/foo.jar` would become a fabricated candidate for every
      // bare-host resource type.
      if !has_path(&api) {
        continue;
      }

      let base = base_text(&api);
      if !url_starts_with_base(text, &base) {
        continue;
      }

      if best
        .as_deref()
        .is_none_or(|current| base.len() > current.len())
      {
        best = Some(base);
        matched.clear();
      } else if best.as_deref() != Some(base.as_str()) {
        continue;
      }

      if !matched.contains(resource_type) {
        matched.push(*resource_type);
      }
    }
  }

  best.map(|base| (base, matched))
}

/// Bases a URL may currently point at, in reverse-derivation order: the
/// official host, BMCLAPI, then every configured mirror.
///
/// The official host only counts when it is Mojang's own. A third-party Maven
/// host (fabricmc, forge, neoforged, ...) is where that artifact really lives,
/// not a mirror of a Mojang path, so a URL there must never be rewritten onto
/// a Mojang-style candidate (see `OFFICIAL_PREFIXES`).
fn derivation_bases(resource_type: ResourceType, config: &LauncherConfig) -> Vec<Url> {
  let mut bases = Vec::new();

  if let Ok(api) = get_download_api(SourceType::Official, resource_type) {
    if OFFICIAL_PREFIXES
      .iter()
      .any(|prefix| api.as_str().starts_with(prefix))
    {
      bases.push(api);
    }
  }

  if let Ok(api) = get_download_api(SourceType::BMCLAPIMirror, resource_type) {
    bases.push(api);
  }

  for mirror in configured_mirrors(config) {
    if let Some(api) = mirror_download_api(&mirror, resource_type) {
      bases.push(api);
    }
  }

  bases
}

/// Bases to try for a URL whose suffix below its current base is `suffix`.
///
/// Maven-family resource types get their namespace-specific Maven hosts first,
/// then their own official base, then BMCLAPI and the user mirrors.
fn alternative_bases(
  resource_type: ResourceType,
  suffix: &str,
  config: &LauncherConfig,
) -> Vec<String> {
  let mut bases: Vec<String> = Vec::new();

  if is_maven_family(resource_type) {
    for repository in maven_priority(suffix) {
      push_unique_base(&mut bases, repository);
    }
  }

  if let Ok(api) = get_download_api(SourceType::Official, resource_type) {
    push_unique_base(&mut bases, &base_text(&api));
  }

  if let Ok(api) = get_download_api(SourceType::BMCLAPIMirror, resource_type) {
    push_unique_base(&mut bases, &base_text(&api));
  }

  for mirror in configured_mirrors(config) {
    if let Some(api) = mirror_download_api(&mirror, resource_type) {
      push_unique_base(&mut bases, &base_text(&api));
    }
  }

  bases
}

/// Resource types whose objects live in a Maven repository, so any Maven host
/// that proxies the same coordinates can serve them.
fn is_maven_family(resource_type: ResourceType) -> bool {
  matches!(
    resource_type,
    ResourceType::Libraries
      | ResourceType::FabricMaven
      | ResourceType::ForgeMaven
      | ResourceType::ForgeMavenNew
      | ResourceType::NeoforgeMaven
      | ResourceType::QuiltMaven
      | ResourceType::CleanroomMaven
  )
}

/// Maven hosts to try for `suffix`, most specific namespace first: a
/// `net/fabricmc` artifact is most likely on the Fabric Maven, while
/// `libraries.minecraft.net` mirrors some of those coordinates as well.
fn maven_priority(suffix: &str) -> &'static [&'static str] {
  if suffix.starts_with("/net/fabricmc/") {
    &[MAVEN_FABRIC, MAVEN_LIBRARIES, MAVEN_CENTRAL]
  } else if suffix.starts_with("/net/minecraftforge/") {
    &[MAVEN_FORGE, MAVEN_LIBRARIES, MAVEN_FABRIC, MAVEN_CENTRAL]
  } else if suffix.starts_with("/net/neoforged/") {
    &[MAVEN_NEOFORGE, MAVEN_LIBRARIES, MAVEN_FABRIC, MAVEN_CENTRAL]
  } else {
    &[MAVEN_LIBRARIES, MAVEN_FABRIC, MAVEN_CENTRAL]
  }
}

/// Scheme, host and path of a download base, without the trailing slash that
/// separates the base from the object suffix.
fn base_text(api: &Url) -> String {
  api.as_str().trim_end_matches('/').to_string()
}

/// True when the base carries a path of its own.
fn has_path(api: &Url) -> bool {
  !api.path().trim_matches('/').is_empty()
}

fn url_starts_with_base(url: &str, base: &str) -> bool {
  url == base
    || url
      .strip_prefix(base)
      .is_some_and(|rest| rest.starts_with('/'))
}

fn push_unique_base(bases: &mut Vec<String>, base: &str) {
  let base = base.trim_end_matches('/');
  if base.is_empty() || bases.iter().any(|existing| existing == base) {
    return;
  }
  bases.push(base.to_string());
}

fn dedupe(urls: Vec<Url>) -> Vec<Url> {
  let mut seen: HashSet<String> = HashSet::new();
  urls
    .into_iter()
    .filter(|url| seen.insert(url.as_str().to_string()))
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config_with(mirrors: &[(&str, DownloadMirrorKind, bool)]) -> LauncherConfig {
    let mut config = LauncherConfig::default();
    config.download.source.custom_mirror_url = String::new();
    config.download.source.strategy = "custom".to_string();
    config.download.source.mirrors = mirrors
      .iter()
      .map(|(url, kind, enabled)| DownloadMirrorConfig {
        url: (*url).to_string(),
        kind: *kind,
        enabled: *enabled,
      })
      .collect();
    config
  }

  /// Default configuration of a user in mainland China: BMCLAPI is preferred
  /// and no custom mirror is set up.
  fn china_config() -> LauncherConfig {
    let mut config = LauncherConfig::default();
    config.download.source.mirrors.clear();
    config.download.source.custom_mirror_url = String::new();
    config.basic_info.is_china_mainland_ip = true;
    config
  }

  fn position(list: &[Url], expected: &str) -> Option<usize> {
    list
      .iter()
      .position(|candidate| candidate.as_str() == expected)
  }

  #[test]
  fn legacy_base_url_still_produces_a_mirror() {
    let mut config = LauncherConfig::default();
    config.download.source.mirrors.clear();
    config.download.source.custom_mirror_url = "https://mirror.example".to_string();
    let mirrors = configured_mirrors(&config);
    assert_eq!(mirrors.len(), 1);
    assert_eq!(mirrors[0].url, "https://mirror.example");
    assert_eq!(mirrors[0].kind, DownloadMirrorKind::PclPrefix);
  }

  #[test]
  fn disabled_and_blank_mirrors_are_ignored() {
    let config = config_with(&[
      ("https://a.example", DownloadMirrorKind::PclPrefix, false),
      ("   ", DownloadMirrorKind::PclPrefix, true),
      ("https://b.example", DownloadMirrorKind::BmclapiFlat, true),
    ]);
    let mirrors = configured_mirrors(&config);
    assert_eq!(mirrors.len(), 1);
    assert_eq!(mirrors[0].url, "https://b.example");
  }

  #[test]
  fn candidates_start_with_the_original_url() {
    let config = config_with(&[("https://a.example", DownloadMirrorKind::PclPrefix, true)]);
    let url = Url::parse("https://piston-meta.mojang.com/v1/packages/aa/x.json").unwrap();
    let list = candidates(&url, &[ResourceType::PistonMeta], &config);
    assert_eq!(list[0], url);
  }

  #[test]
  fn pcl_prefix_mirror_rewrites_official_urls() {
    let config = config_with(&[("https://a.example", DownloadMirrorKind::PclPrefix, true)]);
    let url =
      Url::parse("https://libraries.minecraft.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar").unwrap();
    let list = candidates(&url, &[ResourceType::Libraries], &config);
    assert!(list.iter().any(|candidate| candidate.as_str()
      == "https://a.example/libraries/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"));
  }

  #[test]
  fn flat_mirror_uses_bmclapi_paths() {
    let config = config_with(&[("https://b.example", DownloadMirrorKind::BmclapiFlat, true)]);
    let url = Url::parse(
      "https://libraries.minecraft.net/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
    )
    .unwrap();
    let list = candidates(&url, &[ResourceType::Libraries], &config);
    assert!(list.iter().any(|candidate| candidate.as_str()
      == "https://b.example/maven/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"));
  }

  #[test]
  fn flat_mirror_maps_the_version_manifest() {
    let config = config_with(&[("https://b.example", DownloadMirrorKind::BmclapiFlat, true)]);
    let url = Url::parse("https://launchermeta.mojang.com/mc/game/version_manifest.json").unwrap();
    let list = candidates(&url, &[ResourceType::VersionManifest], &config);
    assert!(
      list
        .iter()
        .any(|candidate| candidate.as_str() == "https://b.example/mc/game/version_manifest.json")
    );
  }

  #[test]
  fn third_party_maven_is_never_rewritten_to_a_mojang_path() {
    let config = config_with(&[("https://b.example", DownloadMirrorKind::BmclapiFlat, true)]);
    let url =
      Url::parse("https://maven.fabricmc.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar").unwrap();
    let list = candidates(&url, &[ResourceType::FabricMaven], &config);
    assert_eq!(list[0], url);
    assert!(
      list
        .iter()
        .all(|candidate| !candidate.as_str().contains("libraries.minecraft.net"))
    );
  }

  #[test]
  fn candidates_are_deduplicated() {
    let config = config_with(&[
      ("https://b.example", DownloadMirrorKind::BmclapiFlat, true),
      ("https://b.example", DownloadMirrorKind::BmclapiFlat, true),
    ]);
    let url = Url::parse("https://libraries.minecraft.net/a/b.jar").unwrap();
    let list = candidates(&url, &[ResourceType::Libraries], &config);
    let mut unique = list.clone();
    unique.dedup();
    assert_eq!(unique.len(), list.len());
  }

  #[test]
  fn bmclapi_maven_url_derives_libraries_and_fabric_candidates() {
    let config = china_config();
    let url =
      Url::parse("https://bmclapi2.bangbang93.com/maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar")
        .unwrap();
    let list = candidates(&url, &[], &config);
    assert_eq!(list[0], url);

    let libraries = position(
      &list,
      "https://libraries.minecraft.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
    )
    .expect("libraries candidate");
    let fabric = position(
      &list,
      "https://maven.fabricmc.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
    )
    .expect("fabric maven candidate");
    assert!(libraries < fabric);
  }

  #[test]
  fn fabric_namespace_prefers_the_fabric_maven() {
    let config = china_config();
    let url = Url::parse(
      "https://bmclapi2.bangbang93.com/maven/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
    )
    .unwrap();
    let list = candidates(&url, &[ResourceType::Libraries], &config);
    let fabric = position(
      &list,
      "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
    )
    .expect("fabric maven candidate");
    let libraries = position(
      &list,
      "https://libraries.minecraft.net/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
    )
    .expect("libraries candidate");
    assert!(fabric < libraries);
  }

  #[test]
  fn bmclapi_assets_url_derives_the_official_asset_host() {
    let config = china_config();
    let url = Url::parse(
      "https://bmclapi2.bangbang93.com/assets/ab/cdef0123456789abcdef0123456789abcdef0123",
    )
    .unwrap();
    let list = candidates(&url, &[], &config);
    assert_eq!(list[0], url);
    assert!(
      position(
        &list,
        "https://resources.download.minecraft.net/ab/cdef0123456789abcdef0123456789abcdef0123",
      )
      .is_some()
    );
  }

  #[test]
  fn near_prefix_and_unknown_hosts_do_not_fabricate_candidates() {
    let config = china_config();

    let near_prefix = Url::parse("https://bmclapi2.bangbang93.com/mavenX/foo.jar").unwrap();
    let list = candidates(&near_prefix, &[], &config);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0], near_prefix);

    let third_party =
      Url::parse("https://repo.example.invalid/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar").unwrap();
    let list = candidates(&third_party, &[], &config);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0], third_party);
  }

  #[test]
  fn base_only_urls_do_not_panic() {
    let config = china_config();
    let url = Url::parse("https://bmclapi2.bangbang93.com/maven/").unwrap();
    let list = candidates(&url, &[], &config);
    assert_eq!(list[0], url);
    assert!(position(&list, "https://libraries.minecraft.net/").is_some());
  }

  #[test]
  fn custom_mirror_urls_derive_official_candidates() {
    let config = config_with(&[("https://b.example", DownloadMirrorKind::BmclapiFlat, true)]);
    let url = Url::parse("https://b.example/maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar").unwrap();
    let list = candidates(&url, &[ResourceType::Libraries], &config);
    assert_eq!(list[0], url);
    assert!(
      position(
        &list,
        "https://libraries.minecraft.net/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
      )
      .is_some()
    );
  }
}
