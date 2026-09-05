//! Host-filesystem side of the "skill folders" setting: resolves sbx's
//! shared agent-skills store directory and copies user-picked folders into
//! it. Pure host filesystem operations only — no `sbx` CLI, no sandbox.

use std::io;
use std::path::{Path, PathBuf};

const STORE_DIR_OVERRIDE_ENV: &str = "OVERNIGHT_SKILL_STORE_DIR";

/// Resolves sbx's shared agent-skills store directory. Checks
/// `OVERNIGHT_SKILL_STORE_DIR` first (tests point this at a temp dir),
/// otherwise falls back to sbx's own per-OS default location.
pub fn skill_store_dir() -> PathBuf {
  if let Ok(dir) = std::env::var(STORE_DIR_OVERRIDE_ENV) {
    return PathBuf::from(dir);
  }
  resolve_default_skill_store_dir(std::env::var("HOME").ok(), std::env::var("XDG_STATE_HOME").ok(), std::env::var("LOCALAPPDATA").ok())
}

/// Per-OS default, parameterized on the env vars it depends on so it's
/// testable without touching real process environment.
fn resolve_default_skill_store_dir(home: Option<String>, xdg_state_home: Option<String>, local_app_data: Option<String>) -> PathBuf {
  #[cfg(target_os = "macos")]
  {
    let _ = (&xdg_state_home, &local_app_data);
    PathBuf::from(home.unwrap_or_default()).join("Library/Application Support/com.docker.sandboxes/sandboxes/agent-skills")
  }

  #[cfg(target_os = "windows")]
  {
    let _ = (&home, &xdg_state_home);
    let base = local_app_data.unwrap_or_default();
    PathBuf::from(base).join("DockerSandboxes").join("sandboxes").join("state").join("agent-skills")
  }

  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  {
    let _ = &local_app_data;
    let base = xdg_state_home.unwrap_or_else(|| format!("{}/.local/state", home.unwrap_or_default()));
    PathBuf::from(base).join("sandboxes/sandboxes/agent-skills")
  }
}

/// Recursively copies `source` into `store_dir` as a subdirectory named
/// after `source`'s own directory name. Returns the destination path.
pub fn copy_folder_into_store(store_dir: &Path, source: &Path) -> io::Result<PathBuf> {
  let name = source
    .file_name()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("no folder name in {source:?}")))?;
  let dest = store_dir.join(name);
  copy_dir_recursive(source, &dest)?;
  Ok(dest)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> io::Result<()> {
  std::fs::create_dir_all(dest)?;
  for entry in std::fs::read_dir(src)? {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let dest_path = dest.join(entry.file_name());
    if file_type.is_dir() {
      copy_dir_recursive(&entry.path(), &dest_path)?;
    } else if file_type.is_file() {
      std::fs::copy(entry.path(), &dest_path)?;
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  #[test]
  fn override_env_var_wins_over_default() {
    let dir = std::env::temp_dir().join(format!("overnight-skill-store-{}", uuid::Uuid::new_v4()));
    std::env::set_var(STORE_DIR_OVERRIDE_ENV, &dir);
    assert_eq!(skill_store_dir(), dir);
    std::env::remove_var(STORE_DIR_OVERRIDE_ENV);
  }

  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  #[test]
  fn linux_prefers_xdg_state_home_when_set() {
    let resolved = resolve_default_skill_store_dir(Some("/home/agent".to_string()), Some("/custom/state".to_string()), None);
    assert_eq!(resolved, PathBuf::from("/custom/state/sandboxes/sandboxes/agent-skills"));
  }

  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  #[test]
  fn linux_falls_back_to_home_local_state() {
    let resolved = resolve_default_skill_store_dir(Some("/home/agent".to_string()), None, None);
    assert_eq!(resolved, PathBuf::from("/home/agent/.local/state/sandboxes/sandboxes/agent-skills"));
  }

  #[test]
  fn copy_folder_into_store_copies_nested_contents() {
    let root = std::env::temp_dir().join(format!("overnight-skill-copy-{}", uuid::Uuid::new_v4()));
    let source = root.join("my-skill");
    let store_dir = root.join("store");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("SKILL.md"), "# skill").unwrap();
    fs::write(source.join("nested").join("helper.txt"), "helper").unwrap();
    fs::create_dir_all(&store_dir).unwrap();

    let dest = copy_folder_into_store(&store_dir, &source).unwrap();

    assert_eq!(dest, store_dir.join("my-skill"));
    assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "# skill");
    assert_eq!(fs::read_to_string(dest.join("nested").join("helper.txt")).unwrap(), "helper");

    fs::remove_dir_all(&root).unwrap();
  }
}
