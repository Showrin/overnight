//! Tails a configurable log file for the Developer > Daemon Logs page.
//!
//! The path isn't hardcoded because it's OS-specific (see `default_path`),
//! and on non-Windows platforms it's still an unverified guess. The real
//! path is stored as a setting so it can be corrected per machine
//! regardless of what `default_path` guesses.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Only the last chunk of the file is read, so a huge log doesn't get
/// loaded whole into memory/IPC.
const TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DaemonLogResult {
  pub exists: bool,
  /// The file actually tailed — may differ from the configured path when
  /// that path is a directory (the most recently modified file inside it
  /// is used).
  pub resolved_path: Option<String>,
  pub content: String,
  pub truncated: bool,
}

/// OS-specific guess at where the sbx daemon's log file lives. Confirmed
/// against a real Windows install: `sbxd` writes a plain `daemon.log` under
/// `%LOCALAPPDATA%\DockerSandboxes\sandboxes\state\sandboxd\`. The
/// macOS/Linux paths are still an **UNVERIFIED** best-effort guess (no real
/// `sbx` install available there in this dev environment — see
/// `sbx/mod.rs`'s top doc comment) — treat those as a starting point the
/// user can override, not a confirmed default.
pub fn default_path() -> String {
  #[cfg(target_os = "windows")]
  {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_default();
    format!("{base}\\DockerSandboxes\\sandboxes\\state\\sandboxd\\daemon.log")
  }
  #[cfg(target_os = "macos")]
  {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/Library/Logs/com.docker.sandboxes/sandboxes/auditkit")
  }
  #[cfg(target_os = "linux")]
  {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/.local/state/sandboxes/sandboxes/auditkit")
  }
}

pub fn read_tail(configured_path: &str) -> std::io::Result<DaemonLogResult> {
  match resolve_log_file(Path::new(configured_path)) {
    Some(file) => {
      let (content, truncated) = tail_file(&file, TAIL_BYTES)?;
      Ok(DaemonLogResult { exists: true, resolved_path: Some(file.to_string_lossy().into_owned()), content, truncated })
    }
    None => Ok(DaemonLogResult { exists: false, resolved_path: None, content: String::new(), truncated: false }),
  }
}

/// A file is tailed directly; a directory tails the most recently modified
/// file inside it (sbx's audit log rotates into timestamped files, so the
/// newest one has the latest activity).
fn resolve_log_file(path: &Path) -> Option<PathBuf> {
  if path.is_file() {
    return Some(path.to_path_buf());
  }
  if path.is_dir() {
    return fs::read_dir(path)
      .ok()?
      .filter_map(|entry| entry.ok())
      .filter(|entry| entry.path().is_file())
      .max_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok())
      .map(|entry| entry.path());
  }
  None
}

fn tail_file(path: &Path, max_bytes: u64) -> std::io::Result<(String, bool)> {
  let mut file = File::open(path)?;
  let len = file.metadata()?.len();
  let truncated = len > max_bytes;
  if truncated {
    file.seek(SeekFrom::Start(len - max_bytes))?;
  }
  let mut buf = Vec::new();
  file.read_to_end(&mut buf)?;
  let text = String::from_utf8_lossy(&buf).into_owned();
  // Seeking into the middle of the file likely landed mid-line; drop the
  // first (possibly partial) line so the tail starts clean.
  let text = if truncated { text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or(text) } else { text };
  Ok((text, truncated))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn missing_path_reports_not_found() {
    let result = read_tail("/nonexistent/path/daemon.log").unwrap();
    assert!(!result.exists);
    assert_eq!(result.content, "");
  }

  #[test]
  fn tails_a_small_file_whole() {
    let dir = std::env::temp_dir().join(format!("overnight-daemon-log-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("daemon.log");
    fs::write(&path, "line one\nline two\n").unwrap();

    let result = read_tail(path.to_str().unwrap()).unwrap();
    assert!(result.exists);
    assert!(!result.truncated);
    assert_eq!(result.content, "line one\nline two\n");

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn tails_a_directory_by_picking_the_newest_file() {
    let dir = std::env::temp_dir().join(format!("overnight-daemon-log-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("old.jsonl"), "old\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::write(dir.join("new.jsonl"), "new\n").unwrap();

    let result = read_tail(dir.to_str().unwrap()).unwrap();
    assert!(result.exists);
    assert_eq!(result.content, "new\n");
    assert!(result.resolved_path.unwrap().ends_with("new.jsonl"));

    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn truncates_large_files_to_the_tail() {
    let dir = std::env::temp_dir().join(format!("overnight-daemon-log-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("daemon.log");
    let mut file = File::create(&path).unwrap();
    for i in 0..50_000 {
      writeln!(file, "line {i}").unwrap();
    }
    drop(file);

    let result = read_tail(path.to_str().unwrap()).unwrap();
    assert!(result.truncated);
    assert!(result.content.len() as u64 <= TAIL_BYTES);
    assert!(result.content.ends_with("line 49999\n"));
    assert!(!result.content.starts_with("line 0\n"));

    fs::remove_dir_all(dir).unwrap();
  }
}
