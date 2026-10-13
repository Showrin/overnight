//! Overnight's own Chrome instance for browser tests, kept apart from the
//! user's everyday profile with a dedicated `--user-data-dir`. The user
//! installs the Claude extension in it once (and signs in to claude.ai and
//! any test accounts); after that the runner starts it whenever a test
//! needs it, so the host agent never touches the everyday profile.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::db::{settings, DbPool};

const CHROME_PATH_SETTING: &str = "testing_chrome_path";
const EXTENSION_URL: &str = "https://chromewebstore.google.com/detail/claude/fcoeoabgfenejglbffodgkkbkcdhcgfn";
const CLAUDE_LOGIN_URL: &str = "https://claude.ai/login";
/// How long a freshly launched Chrome gets for the extension to connect
/// before the host agent starts.
const STARTUP_GRACE: Duration = Duration::from_secs(8);

#[derive(Debug, Serialize)]
pub struct ChromeStatus {
  /// The Chrome executable in use, or `None` if none was found.
  pub chrome_path: Option<String>,
  /// The user's own override, if set.
  pub chrome_path_override: Option<String>,
  pub profile_dir: String,
  /// Whether the testing profile has been opened at least once.
  pub profile_exists: bool,
  pub running: bool,
}

pub fn profile_dir(app: &AppHandle) -> Result<PathBuf, String> {
  Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("chrome-testing-profile"))
}

fn chrome_path_override(pool: &DbPool) -> Option<String> {
  let conn = pool.get().ok()?;
  settings::get(&conn, CHROME_PATH_SETTING).ok().flatten().filter(|p| !p.trim().is_empty())
}

pub fn save_chrome_path_override(pool: &DbPool, path: Option<&str>) -> Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, CHROME_PATH_SETTING, path.map(str::trim).unwrap_or_default()).map_err(|e| e.to_string())
}

fn find_chrome(path_override: Option<&str>) -> Option<PathBuf> {
  if let Some(path) = path_override {
    return Some(PathBuf::from(path)).filter(|p| p.is_file());
  }
  default_chrome_candidates().into_iter().find(|p| p.is_file())
}

fn default_chrome_candidates() -> Vec<PathBuf> {
  #[cfg(target_os = "windows")]
  {
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
      .iter()
      .filter_map(|var| std::env::var(var).ok())
      .map(|base| PathBuf::from(base).join(r"Google\Chrome\Application\chrome.exe"))
      .collect()
  }
  #[cfg(target_os = "macos")]
  {
    vec![PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")]
  }
  #[cfg(not(any(target_os = "windows", target_os = "macos")))]
  {
    ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/opt/google/chrome/chrome"].map(PathBuf::from).to_vec()
  }
}

fn user_data_dir_arg(profile_dir: &Path) -> String {
  format!("--user-data-dir={}", profile_dir.display())
}

fn launch_args(profile_dir: &Path, urls: &[&str]) -> Vec<String> {
  let mut args = vec![user_data_dir_arg(profile_dir), "--no-first-run".to_string(), "--no-default-browser-check".to_string()];
  args.extend(urls.iter().map(|u| u.to_string()));
  args
}

/// Whether a process command line belongs to the testing Chrome. Paths are
/// case-insensitive on Windows, so compare that way everywhere — a false
/// match would need two profile dirs differing only by case.
fn cmd_uses_profile(cmd: &[OsString], profile_dir: &Path) -> bool {
  let wanted = user_data_dir_arg(profile_dir).to_lowercase();
  let wanted = wanted.trim_end_matches(['/', '\\']);
  cmd.iter().any(|arg| arg.to_string_lossy().to_lowercase().trim_matches('"').trim_end_matches(['/', '\\']) == wanted)
}

fn is_running(profile_dir: &Path) -> bool {
  use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
  let mut sys = System::new();
  sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new().with_cmd(UpdateKind::Always));
  sys.processes().values().any(|p| cmd_uses_profile(p.cmd(), profile_dir))
}

pub fn status(app: &AppHandle, pool: &DbPool) -> Result<ChromeStatus, String> {
  let dir = profile_dir(app)?;
  let path_override = chrome_path_override(pool);
  Ok(ChromeStatus {
    chrome_path: find_chrome(path_override.as_deref()).map(|p| p.display().to_string()),
    chrome_path_override: path_override,
    profile_exists: dir.is_dir(),
    running: is_running(&dir),
    profile_dir: dir.display().to_string(),
  })
}

/// Opens the testing Chrome. `setup` opens the extension's store page and
/// the claude.ai sign-in, for first-time setup.
pub fn open(app: &AppHandle, pool: &DbPool, setup: bool) -> Result<(), String> {
  let dir = profile_dir(app)?;
  let chrome = find_chrome(chrome_path_override(pool).as_deref())
    .ok_or("Chrome wasn't found — set its path in Settings > Browser testing")?;
  std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
  link_native_host(&dir);
  let urls: &[&str] = if setup { &[EXTENSION_URL, CLAUDE_LOGIN_URL] } else { &[] };
  std::process::Command::new(&chrome)
    .args(launch_args(&dir, urls))
    .spawn()
    .map_err(|e| format!("couldn't start {}: {e}", chrome.display()))?;
  Ok(())
}

/// Called before each test: makes sure the testing Chrome is up, starting
/// it (and giving the extension time to connect) if it isn't.
pub async fn ensure_running(app: &AppHandle, pool: &DbPool) -> Result<(), String> {
  let dir = profile_dir(app)?;
  if !dir.is_dir() {
    return Err("the testing browser isn't set up yet — open Settings > Browser testing and set it up".to_string());
  }
  if is_running(&dir) {
    return Ok(());
  }
  open(app, pool, false)?;
  tokio::time::sleep(STARTUP_GRACE).await;
  Ok(())
}

/// On Windows, Chrome finds Claude Code's native messaging host through
/// the registry for every user-data-dir. Elsewhere it only looks inside the
/// user-data-dir, so copy Claude Code's host manifest in from the default
/// profile (written there by the first `claude --chrome` run).
#[cfg(not(target_os = "windows"))]
fn link_native_host(profile_dir: &Path) {
  const MANIFEST: &str = "com.anthropic.claude_code_browser_extension.json";
  let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return };
  #[cfg(target_os = "macos")]
  let source = home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts").join(MANIFEST);
  #[cfg(not(target_os = "macos"))]
  let source = home.join(".config/google-chrome/NativeMessagingHosts").join(MANIFEST);
  if !source.is_file() {
    return;
  }
  let dest_dir = profile_dir.join("NativeMessagingHosts");
  if let Err(e) = std::fs::create_dir_all(&dest_dir).and_then(|_| std::fs::copy(&source, dest_dir.join(MANIFEST))) {
    log::warn!("couldn't copy Claude's native messaging host into the testing profile: {e}");
  }
}

#[cfg(target_os = "windows")]
fn link_native_host(_profile_dir: &Path) {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn launch_args_isolate_the_profile() {
    let dir = Path::new("/data/chrome-testing-profile");
    let args = launch_args(dir, &["https://a.test"]);
    assert_eq!(args[0], "--user-data-dir=/data/chrome-testing-profile");
    assert!(args.contains(&"--no-first-run".to_string()));
    assert_eq!(args.last().unwrap(), "https://a.test");
  }

  #[test]
  fn recognizes_the_testing_chrome_only() {
    let dir = Path::new(r"C:\Users\me\AppData\Roaming\overnight\chrome-testing-profile");
    let ours = [OsString::from("chrome.exe"), OsString::from(r"--user-data-dir=C:\Users\Me\AppData\Roaming\overnight\chrome-testing-profile")];
    let daily = [OsString::from("chrome.exe"), OsString::from("--profile-directory=Default")];
    let other = [OsString::from("chrome.exe"), OsString::from(r"--user-data-dir=C:\Users\me\AppData\Roaming\overnight\chrome-testing-profile-2")];
    assert!(cmd_uses_profile(&ours, dir));
    assert!(!cmd_uses_profile(&daily, dir));
    assert!(!cmd_uses_profile(&other, dir));
  }

  #[test]
  fn override_must_exist() {
    assert!(find_chrome(Some("/definitely/not/chrome")).is_none());
  }
}
