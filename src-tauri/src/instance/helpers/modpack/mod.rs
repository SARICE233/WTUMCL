pub mod curseforge;
pub mod export;
pub mod import;
pub mod modrinth;
pub mod multimc;

// Windows test binaries do not receive the Common Controls v6 manifest that
// `tauri-build` embeds into application binaries. Without it the Windows loader
// binds comctl32 v5, and the test harness of this crate fails to start with
// STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) because tauri-runtime-wry imports
// `TaskDialogIndirect`. Linking tauri-build's generated `resource.lib` (which
// carries the same manifest) in test builds only fixes that without touching
// application binaries.
#[cfg(all(test, windows))]
#[link(name = "resource", kind = "static")]
unsafe extern "C" {}
