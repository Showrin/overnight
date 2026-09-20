//! Periodic (and pre-stop/pre-delete) backups of a sandbox's `~/.claude`
//! and `<workspace>/.git` directories, keeping only the newest
//! `KEEP_BACKUPS_PER_SANDBOX` per sandbox. See `db::backups` for storage
//! and `run_scheduler` for the interval loop started from `lib.rs::setup`.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::db::models::{now_millis, SandboxBackup};
use crate::db::{backups, sandboxes, settings, DbPool};

pub const CLAUDE_SOURCE_PATH: &str = "/home/agent/.claude";
/// Subdirectory of `app_data_dir` every sandbox's backups live under —
/// shared with `commands::open_backups_root_folder` so the global Backups
/// page can open the same root a backup's `host_dir` is nested inside.
pub const BACKUPS_ROOT_DIR_NAME: &str = "sandbox-backups";
const KEEP_BACKUPS_PER_SANDBOX: i64 = 10;
const SCHEDULER_TICK_SECS: u64 = 60;

pub const BACKUP_INTERVAL_KEY: &str = "backup_interval_minutes";
pub const DEFAULT_BACKUP_INTERVAL_MINUTES: i64 = 15;
const BACKUP_LAST_RUN_KEY: &str = "backup_last_run_at";
/// Gates only `run_scheduler`'s periodic ticking — manual backups and the
/// pre-stop/pre-delete consent backup are unaffected either way. Absent
/// (e.g. an existing install that predates this setting) means enabled,
/// matching the scheduler's behavior before this toggle existed.
pub const AUTO_BACKUP_ENABLED_KEY: &str = "auto_backup_enabled";

/// Which halves of a sandbox a backup (or restore) covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupScope {
  Claude,
  Git,
  All,
}

impl BackupScope {
  pub fn parse(raw: &str) -> std::result::Result<Self, String> {
    match raw {
      "claude" => Ok(Self::Claude),
      "git" => Ok(Self::Git),
      "all" => Ok(Self::All),
      other => Err(format!("invalid backup scope: {other}")),
    }
  }

  pub fn wants_claude(self) -> bool {
    matches!(self, Self::Claude | Self::All)
  }

  pub fn wants_git(self) -> bool {
    matches!(self, Self::Git | Self::All)
  }
}

/// Which sandboxes currently have a backup copy in flight, keyed by
/// sandbox id — managed as `Mutex<BackupState>` app state, same pattern as
/// `sbx::HostMonitor`/`sbx::SandboxMonitor`. Lets the frontend show a
/// progress indicator and disable Stop/Delete while a copy is running.
#[derive(Default)]
pub struct BackupState {
  active: HashMap<String, ActiveBackup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveBackup {
  pub sandbox_id: String,
  pub trigger: String,
  pub started_at: i64,
}

pub fn active_backups(app: &AppHandle) -> Vec<ActiveBackup> {
  let Some(state) = app.try_state::<Mutex<BackupState>>() else {
    return Vec::new();
  };
  let Ok(state) = state.lock() else {
    return Vec::new();
  };
  state.active.values().cloned().collect()
}

/// Removes `sandbox_id` from the active-backups map when dropped, so
/// `backup_sandbox` clears its in-progress marker on every exit path
/// (success, error, or an early `?`) without repeating that call at each
/// return site.
struct ActiveBackupGuard {
  app: AppHandle,
  sandbox_id: String,
}

impl Drop for ActiveBackupGuard {
  fn drop(&mut self) {
    if let Some(state) = self.app.try_state::<Mutex<BackupState>>() {
      if let Ok(mut state) = state.lock() {
        state.active.remove(&self.sandbox_id);
      }
    }
  }
}

/// Copies `sandbox_id`'s `~/.claude` and (if it has a git workspace)
/// `.git` directory to a fresh host folder, records the backup, and prunes
/// anything beyond the newest `KEEP_BACKUPS_PER_SANDBOX`. A missing `.git`
/// (or a failed copy of either half) doesn't cancel the other half — the
/// two must stay independently importable. Every call site treats this as
/// best-effort and only logs the error.
pub async fn backup_sandbox(
  app: &AppHandle,
  pool: &DbPool,
  sandbox_id: &str,
  trigger: &str,
  scope: BackupScope,
) -> std::result::Result<SandboxBackup, String> {
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, sandbox_id).map_err(|e| e.to_string())?
  };
  if sandbox.status != "running" {
    return Err("sandbox not running".to_string());
  }
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox yet".to_string())?;
  let sandbox_label = sandbox.name.clone().unwrap_or_else(|| name.clone());

  if let Some(state) = app.try_state::<Mutex<BackupState>>() {
    if let Ok(mut state) = state.lock() {
      state.active.insert(
        sandbox_id.to_string(),
        ActiveBackup { sandbox_id: sandbox_id.to_string(), trigger: trigger.to_string(), started_at: now_millis() },
      );
    }
  }
  let _guard = ActiveBackupGuard { app: app.clone(), sandbox_id: sandbox_id.to_string() };

  let root = app
    .path()
    .app_data_dir()
    .map_err(|e| e.to_string())?
    .join(BACKUPS_ROOT_DIR_NAME)
    .join(&name)
    .join(now_millis().to_string());

  let claude_dir = root.join("claude");
  let has_claude = if scope.wants_claude() {
    try_copy(app, &name, CLAUDE_SOURCE_PATH, &claude_dir).await
  } else {
    false
  };

  let workspace = crate::sbx::workspace_path(app, &name).await.map_err(|e| e.to_string())?;
  let has_git = if scope.wants_git() {
    match &workspace {
      Some(ws) => try_copy(app, &name, &format!("{ws}/.git"), &root.join("git")).await,
      None => false,
    }
  } else {
    false
  };

  let plan_file_count = count_plan_files(&claude_dir);

  let row = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let inserted = backups::insert(
      &conn,
      sandbox_id,
      Some(&sandbox_label),
      trigger,
      &root.to_string_lossy(),
      has_claude,
      has_git,
      sandbox.base_branch.as_deref(),
      sandbox.current_branch.as_deref(),
      &sandbox.branches,
      plan_file_count,
    )
    .map_err(|e| e.to_string())?;
    // Best-effort: reflects this snapshot's root (containing claude/ and
    // git/) as the sandbox's most recent backup, regardless of trigger —
    // a failure here shouldn't undo an otherwise-successful backup.
    if let Err(e) = sandboxes::record_backup(&conn, sandbox_id, &root.to_string_lossy(), inserted.created_at) {
      log::warn!("backup_sandbox: failed to update last_backup fields for {sandbox_id}: {e}");
    }
    inserted
  };

  prune_old_backups(pool, sandbox_id);

  Ok(row)
}

async fn try_copy(app: &AppHandle, name: &str, remote_path: &str, host_dest: &std::path::Path) -> bool {
  // `host_dest` itself must NOT exist beforehand — sbx cp nests
  // `remote_path`'s basename one level inside an existing destination
  // instead of copying its contents directly (see sbx::cp_from_sandbox).
  // Only its parent needs to be there.
  let Some(parent) = host_dest.parent() else {
    log::warn!("backup_sandbox: {} has no parent directory", host_dest.display());
    return false;
  };
  if let Err(e) = std::fs::create_dir_all(parent) {
    log::warn!("backup_sandbox: failed to create {}: {e}", parent.display());
    return false;
  }
  match crate::sbx::cp_from_sandbox(app, name, remote_path, &host_dest.to_string_lossy()).await {
    Ok(()) => true,
    Err(e) => {
      log::warn!("backup_sandbox: failed to copy {remote_path} from {name}: {e}");
      false
    }
  }
}

/// `~/.claude/plans` is nested inside the just-copied `~/.claude`, so
/// counting `.md` files here needs no extra remote copy. Checks both
/// `claude_dir/plans` and `claude_dir/.claude/plans` since `sbx cp`'s exact
/// nesting behavior is unverified (see `sbx::cp_from_sandbox`).
fn count_plan_files(claude_dir: &std::path::Path) -> i64 {
  let candidates = [claude_dir.join("plans"), claude_dir.join(".claude").join("plans")];
  let Some(plans_dir) = candidates.into_iter().find(|p| p.is_dir()) else {
    return 0;
  };
  std::fs::read_dir(&plans_dir)
    .map(|entries| {
      entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count() as i64
    })
    .unwrap_or(0)
}

fn prune_old_backups(pool: &DbPool, sandbox_id: &str) {
  let conn = match pool.get() {
    Ok(conn) => conn,
    Err(e) => {
      log::warn!("prune_old_backups: failed to get db connection: {e}");
      return;
    }
  };
  let victims = match backups::beyond_limit(&conn, sandbox_id, KEEP_BACKUPS_PER_SANDBOX) {
    Ok(victims) => victims,
    Err(e) => {
      log::warn!("prune_old_backups: failed to list old backups for {sandbox_id}: {e}");
      return;
    }
  };
  for victim in victims {
    // Delete the row even if the directory removal fails: an orphaned
    // directory on disk is harmless, but a stuck row would keep showing a
    // dead backup in the UI forever.
    if let Err(e) = std::fs::remove_dir_all(&victim.host_dir) {
      log::warn!("prune_old_backups: failed to remove {}: {e}", victim.host_dir);
    }
    if let Err(e) = backups::delete(&conn, &victim.id) {
      log::warn!("prune_old_backups: failed to delete backup row {}: {e}", victim.id);
    }
  }
}

fn read_i64_setting(pool: &DbPool, key: &str) -> Option<i64> {
  let conn = pool.get().ok()?;
  settings::get(&conn, key).ok().flatten()?.parse().ok()
}

pub fn is_auto_backup_enabled(pool: &DbPool) -> bool {
  let Ok(conn) = pool.get() else { return true };
  match settings::get(&conn, AUTO_BACKUP_ENABLED_KEY) {
    Ok(Some(raw)) => raw != "false",
    _ => true,
  }
}

/// Ticks every `SCHEDULER_TICK_SECS`, re-reading the interval (and
/// enabled/disabled) settings each time so a change in Settings takes
/// effect on the next tick with no restart. `backup_last_run_at` is a
/// plain settings key (not a new table) so the interval survives app
/// restarts without a double-backup burst on launch. Left untouched while
/// disabled, so re-enabling doesn't itself trigger an immediate backup
/// unless the interval had already elapsed since the last real one.
pub async fn run_scheduler(app: AppHandle, pool: DbPool) {
  loop {
    tokio::time::sleep(std::time::Duration::from_secs(SCHEDULER_TICK_SECS)).await;

    if !is_auto_backup_enabled(&pool) {
      continue;
    }

    let interval_minutes = read_i64_setting(&pool, BACKUP_INTERVAL_KEY).unwrap_or(DEFAULT_BACKUP_INTERVAL_MINUTES);
    let last_run_at = read_i64_setting(&pool, BACKUP_LAST_RUN_KEY).unwrap_or(0);
    let now = now_millis();
    if now - last_run_at < interval_minutes * 60_000 {
      continue;
    }
    if let Ok(conn) = pool.get() {
      let _ = settings::set(&conn, BACKUP_LAST_RUN_KEY, &now.to_string());
    }

    let running_sandboxes = {
      let conn = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
          log::warn!("run_scheduler: failed to get db connection: {e}");
          continue;
        }
      };
      match sandboxes::list(&conn) {
        Ok(list) => list.into_iter().filter(|s| s.status == "running").collect::<Vec<_>>(),
        Err(e) => {
          log::warn!("run_scheduler: failed to list sandboxes: {e}");
          continue;
        }
      }
    };

    for sandbox in running_sandboxes {
      if let Err(e) = backup_sandbox(&app, &pool, &sandbox.id, "scheduled", BackupScope::All).await {
        log::warn!("run_scheduler: backup failed for sandbox {}: {e}", sandbox.id);
        let label = sandbox.name.clone().unwrap_or_else(|| sandbox.id.clone());
        let _ = crate::notifications::notify_plain(&app, "Backup failed", Some(&format!("{label}: {e}")), Some(sandbox.id), false);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_valid_scopes() {
    assert_eq!(BackupScope::parse("claude"), Ok(BackupScope::Claude));
    assert_eq!(BackupScope::parse("git"), Ok(BackupScope::Git));
    assert_eq!(BackupScope::parse("all"), Ok(BackupScope::All));
    assert!(BackupScope::parse("everything").is_err());
  }

  #[test]
  fn scope_gates_which_halves_are_wanted() {
    assert!(BackupScope::Claude.wants_claude());
    assert!(!BackupScope::Claude.wants_git());
    assert!(BackupScope::Git.wants_git());
    assert!(!BackupScope::Git.wants_claude());
    assert!(BackupScope::All.wants_claude());
    assert!(BackupScope::All.wants_git());
  }

  #[test]
  fn count_plan_files_returns_zero_when_missing() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    assert_eq!(count_plan_files(&dir), 0);
  }

  #[test]
  fn count_plan_files_counts_only_markdown() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    let plans = dir.join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    std::fs::write(plans.join("a.md"), "plan a").unwrap();
    std::fs::write(plans.join("b.md"), "plan b").unwrap();
    std::fs::write(plans.join("notes.txt"), "not a plan").unwrap();

    assert_eq!(count_plan_files(&dir), 2);

    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn count_plan_files_checks_nested_claude_dir() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    let plans = dir.join(".claude").join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    std::fs::write(plans.join("a.md"), "plan a").unwrap();

    assert_eq!(count_plan_files(&dir), 1);

    std::fs::remove_dir_all(&dir).ok();
  }

  fn test_pool() -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("overnight-backup-{}.sqlite", crate::db::models::new_id()));
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    crate::db::migrations::migrations().to_latest(&mut conn).unwrap();
    drop(conn);
    let pool = r2d2::Pool::builder()
      .max_size(2)
      .build(r2d2_sqlite::SqliteConnectionManager::file(&path))
      .unwrap();
    (pool, path)
  }

  #[test]
  fn prune_old_backups_removes_rows_and_directories_beyond_limit() {
    let (pool, db_path) = test_pool();
    let conn = pool.get().unwrap();
    let project = crate::db::projects::create(&conn, "Overnight", "/repo", None, None).unwrap();
    let sandbox = crate::db::sandboxes::create(&conn, &project.id, "mount", None, None, "default", None).unwrap();

    let dirs: Vec<_> = (0..12)
      .map(|i| std::env::temp_dir().join(format!("overnight-prune-test-{}-{i}", crate::db::models::new_id())))
      .collect();
    for dir in &dirs {
      std::fs::create_dir_all(dir).unwrap();
    }

    for (i, dir) in dirs.iter().enumerate() {
      let row =
        backups::insert(&conn, &sandbox.id, None, "scheduled", &dir.to_string_lossy(), true, true, None, None, &[], 0).unwrap();
      conn
        .execute("UPDATE sandbox_backups SET created_at = ?1 WHERE id = ?2", rusqlite::params![i as i64, row.id])
        .unwrap();
    }
    drop(conn);

    prune_old_backups(&pool, &sandbox.id);

    let conn = pool.get().unwrap();
    assert_eq!(backups::list_all(&conn).unwrap().len(), 10);
    assert!(!dirs[0].exists());
    assert!(!dirs[1].exists());
    assert!(dirs[2].exists());

    for dir in &dirs[2..] {
      std::fs::remove_dir_all(dir).ok();
    }
    drop(conn);
    drop(pool);
    std::fs::remove_file(&db_path).ok();
  }

  #[test]
  fn auto_backup_enabled_by_default_and_toggleable() {
    let (pool, db_path) = test_pool();
    assert!(is_auto_backup_enabled(&pool));

    {
      let conn = pool.get().unwrap();
      settings::set(&conn, AUTO_BACKUP_ENABLED_KEY, "false").unwrap();
    }
    assert!(!is_auto_backup_enabled(&pool));

    {
      let conn = pool.get().unwrap();
      settings::set(&conn, AUTO_BACKUP_ENABLED_KEY, "true").unwrap();
    }
    assert!(is_auto_backup_enabled(&pool));

    drop(pool);
    std::fs::remove_file(&db_path).ok();
  }
}
