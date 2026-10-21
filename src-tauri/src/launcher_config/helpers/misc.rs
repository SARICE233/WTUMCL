use rand::Rng;
use sjmcl_types::error::SJMCLResult;
use sjmcl_types::partial::{PartialAccess, PartialUpdate};
use std::fs;
#[cfg(not(target_os = "macos"))]
use std::path::Path;
use std::path::{MAIN_SEPARATOR, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::launcher_config::commands::retrieve_custom_background_list;
use crate::launcher_config::models::{
  BasicInfo, BuildType, GameConfig, GameDirectory, LauncherConfig,
};
use crate::utils::fs::calculate_sha256;
use crate::utils::portable::extract_assets;
use crate::{APP_DATA_DIR, EXE_PATH, IS_PORTABLE};

const DOWNLOAD_CACHE_RETENTION_DAYS: u64 = 30;

// Name of the portable game directory created next to the launcher executable.
#[cfg(not(target_os = "macos"))]
const DEFAULT_PORTABLE_GAME_DIR_NAME: &str = "WTUMCL";

impl LauncherConfig {
  pub fn setup_with_app(&mut self, app: &AppHandle) -> SJMCLResult<()> {
    // Resolve build_type and displayed version string.
    let is_dev = cfg!(debug_assertions);
    let pkg_version = app.package_info().version.to_string();
    let build_type = option_env!("WTUMCL_BUILD_TYPE")
      .map(|s| s.parse().unwrap_or(BuildType::Dev))
      .unwrap_or(match (is_dev, pkg_version.as_str()) {
        (true, _) => BuildType::Dev,
        (false, "0.0.0") => BuildType::Nightly,
        (false, "0.0.1") => BuildType::TestBuild,
        _ => BuildType::Release,
      });
    let version = match build_type {
      BuildType::Release => pkg_version,
      _ => build_type.to_string(),
    };

    // Set the default download cache directory if unset or not writable, and create it
    if self.download.cache.directory.as_os_str().is_empty()
      || fs::create_dir_all(&self.download.cache.directory).is_err()
    {
      self.download.cache.directory = app
        .path()
        .resolve::<PathBuf>("Download".into(), BaseDirectory::AppCache)?;
      fs::create_dir_all(&self.download.cache.directory)?;
    }

    // Random pick custom background image if enabled
    if self.appearance.background.random_custom {
      let app_handle = app.clone();
      match retrieve_custom_background_list(app_handle) {
        Ok(backgrounds) if !backgrounds.is_empty() => {
          let mut rng = rand::rng();
          let random_index = rng.random_range(0..backgrounds.len());
          self.appearance.background.choice = backgrounds[random_index].clone();
        }
        _ => {
          self.appearance.background.random_custom = false;
        }
      }
    }

    // Set default local game directories
    if self.local_game_directories.is_empty() {
      let mut dirs = Vec::new();

      #[cfg(target_os = "macos")]
      {
        if let Some(app_data_dir) = APP_DATA_DIR.get() {
          let app_data_subdir = app_data_dir.join("minecraft");
          dirs.push(GameDirectory {
            name: "APP_DATA_SUBDIR".to_string(),
            dir: app_data_subdir,
          });
        }
      }

      #[cfg(not(target_os = "macos"))]
      dirs.push(default_game_directory(
        crate::EXE_DIR.as_path(),
        APP_DATA_DIR
          .get()
          .expect("APP_DATA_DIR initialization failed"),
        is_directory_writable,
      ));

      dirs.push(get_official_minecraft_directory(app));
      self.local_game_directories = dirs;
    }

    for game_dir in &mut self.local_game_directories {
      if (game_dir.name == "CURRENT_DIR" || game_dir.name == "APP_DATA_SUBDIR")
        && !game_dir.dir.exists()
      {
        let _ = fs::create_dir_all(&game_dir.dir);
      }
    }

    // Extract assets if the application is portable
    if *IS_PORTABLE {
      let _ = extract_assets(app);
    }

    self.basic_info = BasicInfo {
      launcher_version: version,
      platform: tauri_plugin_os::platform().to_string(),
      arch: tauri_plugin_os::arch().to_string(),
      os_type: tauri_plugin_os::type_().to_string(),
      platform_version: tauri_plugin_os::version().to_string(),
      exe_sha256: calculate_sha256(&EXE_PATH).unwrap_or_default(),
      is_portable: *IS_PORTABLE,
      is_exe_path_available: check_exe_path_availability(app),
      // below set to default, will be updated later in first time calling `check_full_login_availability`
      is_china_mainland_ip: false,
      allow_full_login_feature: false,
      // build metadata: compile-time constants may be injected by build.rs
      build_type,
      build_commit_sha: option_env!("WTUMCL_COMMIT_SHA").unwrap_or("").to_string(),
    };

    log::info!(
      "Launcher basic info: {}",
      serde_json::to_string(&self.basic_info).unwrap_or_else(|_| format!("{:?}", self.basic_info))
    );

    Ok(())
  }

  pub fn replace_with_preserved(&mut self, new_config: LauncherConfig, preserved_fields: &[&str]) {
    // Preserve some fields when restore or import
    let mut backup_values = Vec::new();
    for key in preserved_fields {
      if let Ok(value) = self.access(key) {
        backup_values.push((key, value));
      }
    }

    *self = new_config;

    for (key, value) in backup_values {
      let _ = self.update(key, &value);
    }
  }
}

fn get_official_minecraft_directory(app: &AppHandle) -> GameDirectory {
  let minecraft_dir: PathBuf;

  #[cfg(target_os = "windows")]
  {
    // Windows: {FOLDERID_RoamingAppData}\.minecraft
    minecraft_dir = app
      .path()
      .resolve::<PathBuf>(".minecraft".into(), BaseDirectory::Data)
      .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Default\AppData\Roaming\.minecraft"));
  }

  #[cfg(target_os = "macos")]
  {
    // macOS: ~/Library/Application Support/minecraft
    minecraft_dir = app
      .path()
      .resolve::<PathBuf>("minecraft".into(), BaseDirectory::Data)
      .unwrap_or_else(|_| PathBuf::from("/Users/Shared/Library/Application Support/minecraft"));
  }

  #[cfg(target_os = "linux")]
  {
    // Linux: ~/.minecraft
    minecraft_dir = app
      .path()
      .resolve::<PathBuf>(".minecraft".into(), BaseDirectory::Home)
      .unwrap_or_else(|_| PathBuf::from("/home/user/.minecraft"));
  }

  GameDirectory {
    name: "OFFICIAL_DIR".to_string(),
    dir: minecraft_dir,
  }
}

// Resolve the default game directory for non-macOS platforms: a portable
// `WTUMCL` folder next to the running executable (i.e. `current_exe().parent()`),
// so that new users do not end up with a game directory under their user profile.
// If the executable directory cannot be written to (e.g. the launcher is installed
// under `Program Files`), fall back to the existing app data subdirectory.
// This is only used when no game directory is configured yet, so existing
// configurations are never migrated or overwritten.
#[cfg(not(target_os = "macos"))]
fn default_game_directory(
  exe_dir: &Path,
  app_data_dir: &Path,
  is_writable: impl Fn(&Path) -> bool,
) -> GameDirectory {
  let portable_dir = exe_dir.join(DEFAULT_PORTABLE_GAME_DIR_NAME);

  if is_writable(&portable_dir) {
    GameDirectory {
      name: "CURRENT_DIR".to_string(),
      dir: portable_dir,
    }
  } else {
    log::warn!(
      "Game directory next to the executable is not writable, falling back to app data: {:?}",
      app_data_dir
    );
    GameDirectory {
      name: "APP_DATA_SUBDIR".to_string(),
      dir: app_data_dir.join(".minecraft"),
    }
  }
}

// Create `dir` if needed and verify that files can actually be written into it.
// Errors are treated as "not writable" so that the caller can fall back instead
// of failing the launcher startup.
#[cfg(not(target_os = "macos"))]
fn is_directory_writable(dir: &Path) -> bool {
  if fs::create_dir_all(dir).is_err() {
    return false;
  }

  let probe = dir.join(format!(".wtumcl-write-probe-{}", std::process::id()));
  let writable = fs::write(&probe, []).is_ok();
  let _ = fs::remove_file(&probe);
  writable
}

pub fn get_global_game_config(app: &AppHandle) -> GameConfig {
  app
    .state::<Mutex<LauncherConfig>>()
    .lock()
    .unwrap()
    .global_game_config
    .clone()
}

// Check if the executable is running from a temporary or non-persistent path
pub fn check_exe_path_availability(app: &AppHandle) -> bool {
  let exe_str = &*EXE_PATH.to_string_lossy();

  if app
    .path()
    .resolve::<PathBuf>("".into(), BaseDirectory::Temp)
    .is_ok_and(|temp| exe_str.starts_with(&*temp.to_string_lossy()))
  {
    return false;
  }

  // generic keywords and OS-specific hard rules
  // modified from: https://github.com/HMCL-dev/HMCL/blob/0a0476b6d32ccd689c7166d25326c1a81cf64564/HMCL/src/main/java/org/jackhuang/hmcl/Launcher.java#L192
  let exe_lower = exe_str.to_lowercase();
  let keywords = ["temp", "cache", "caches", ".cache"];
  for k in keywords {
    let needle = format!("{sep}{k}{sep}", sep = MAIN_SEPARATOR);
    if exe_lower.contains(&needle) {
      return false;
    }
  }

  #[cfg(target_os = "windows")]
  {
    !(exe_str.contains("\\Temporary Internet Files\\")
      || exe_str.contains("\\INetCache\\")
      || exe_str.contains("\\$Recycle.Bin\\"))
  }

  #[cfg(target_os = "linux")]
  {
    !(exe_str.starts_with("/tmp/")
      || exe_str.starts_with("/var/tmp/")
      || exe_str.starts_with("/var/cache/")
      || exe_str.starts_with("/dev/shm/")
      // /run is tmpfs, while /run/media is a common mount point on some Linux distributions.
      || (exe_str.starts_with("/run/") && !exe_str.starts_with("/run/media/"))
      || exe_str.contains("/Trash/"))
  }

  #[cfg(target_os = "macos")]
  {
    !(exe_str.starts_with("/var/folders/")
      || exe_str.starts_with("/private/var/folders/")
      || exe_str.starts_with("/tmp/")
      || exe_str.starts_with("/var/tmp/")
      || exe_str.starts_with("/private/tmp/")
      || exe_str.starts_with("/private/var/tmp/")
      || exe_str.contains("/.Trash/"))
  }
}

pub async fn auto_clear_download_cache(app: &AppHandle) -> SJMCLResult<()> {
  let cache_dir = {
    let config = app.state::<Mutex<LauncherConfig>>();
    let config = config.lock()?;
    config.download.cache.directory.clone()
  };

  let now = SystemTime::now();
  let retention = Duration::from_secs(DOWNLOAD_CACHE_RETENTION_DAYS * 24 * 60 * 60);

  let mut entries = match tokio::fs::read_dir(&cache_dir).await {
    Ok(entries) => entries,
    Err(_) => return Ok(()),
  };
  while let Some(entry) = entries.next_entry().await? {
    let metadata = match entry.metadata().await {
      Ok(metadata) => metadata,
      Err(_) => continue,
    };
    if !metadata.is_file() {
      continue;
    }
    if metadata
      .modified()
      .ok()
      .and_then(|modified| now.duration_since(modified).ok())
      .is_some_and(|age| age > retention)
      && let Err(e) = tokio::fs::remove_file(entry.path()).await
    {
      log::warn!(
        "Failed to remove stale download cache entry {:?}: {}",
        entry.path(),
        e
      );
    }
  }

  Ok(())
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
  use super::*;
  use std::cell::RefCell;
  use std::time::{SystemTime, UNIX_EPOCH};

  fn temp_path(tag: &str) -> PathBuf {
    let unique = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|duration| duration.as_nanos())
      .unwrap_or_default();
    std::env::temp_dir().join(format!(
      "wtumcl-game-dir-{tag}-{}-{unique}",
      std::process::id()
    ))
  }

  #[test]
  fn default_game_directory_joins_exe_directory_with_wtumcl() {
    let exe_dir = Path::new("/opt/wtumcl");
    let app_data_dir = Path::new("/home/user/.local/share/wtumcl");

    let game_dir = default_game_directory(exe_dir, app_data_dir, |_| true);

    assert_eq!(game_dir.name, "CURRENT_DIR");
    assert_eq!(game_dir.dir, exe_dir.join(DEFAULT_PORTABLE_GAME_DIR_NAME));
    assert_eq!(game_dir.dir, exe_dir.join("WTUMCL"));
  }

  #[test]
  fn writability_of_exe_adjacent_wtumcl_directory_is_probed() {
    let exe_dir = Path::new("/opt/wtumcl");
    let app_data_dir = Path::new("/home/user/.local/share/wtumcl");
    let probed = RefCell::new(None);

    default_game_directory(exe_dir, app_data_dir, |candidate| {
      *probed.borrow_mut() = Some(candidate.to_path_buf());
      false
    });

    assert_eq!(
      probed.into_inner(),
      Some(exe_dir.join(DEFAULT_PORTABLE_GAME_DIR_NAME))
    );
  }

  #[test]
  fn falls_back_to_app_data_when_exe_adjacent_directory_is_not_writable() {
    let exe_dir = Path::new("/opt/wtumcl");
    let app_data_dir = Path::new("/home/user/.local/share/wtumcl");

    let game_dir = default_game_directory(exe_dir, app_data_dir, |_| false);

    assert_eq!(game_dir.name, "APP_DATA_SUBDIR");
    assert_eq!(game_dir.dir, app_data_dir.join(".minecraft"));
    assert!(!game_dir.dir.starts_with(exe_dir));
  }

  #[test]
  fn writable_directory_is_created_and_probed_without_leftovers() {
    let dir = temp_path("writable");

    assert!(is_directory_writable(&dir));
    assert!(dir.is_dir());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);

    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn directory_path_blocked_by_a_file_is_not_writable() {
    let dir = temp_path("blocked");
    fs::write(&dir, b"not a directory").unwrap();

    assert!(!is_directory_writable(&dir));

    let _ = fs::remove_file(&dir);
  }

  #[test]
  fn falls_back_when_exe_adjacent_wtumcl_path_is_a_file() {
    let exe_dir = temp_path("exe");
    fs::create_dir_all(&exe_dir).unwrap();
    fs::write(
      exe_dir.join(DEFAULT_PORTABLE_GAME_DIR_NAME),
      b"not a directory",
    )
    .unwrap();
    let app_data_dir = temp_path("app-data");

    let game_dir = default_game_directory(&exe_dir, &app_data_dir, is_directory_writable);

    assert_eq!(game_dir.name, "APP_DATA_SUBDIR");
    assert_eq!(game_dir.dir, app_data_dir.join(".minecraft"));

    let _ = fs::remove_dir_all(&exe_dir);
  }
}
