//! Periodic (and pre-stop/pre-delete) backups of a sandbox's agent home
//! directory (`~/.claude` or `~/.codex`, per `crate::agents::get`) and
//! `<workspace>/.git` directory, keeping only the newest `keep_count`
//! per sandbox. See `db::backups` for storage
//! and `run_scheduler` for the interval loop started from `lib.rs::setup`.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::db::models::{now_millis, SandboxBackup};
use crate::db::{backups, sandboxes, settings, DbPool};

/// Subdirectory of `app_data_dir` every sandbox's backups live under —
/// shared with `commands::open_backups_root_folder` so the global Backups
/// page can open the same root a backup's `host_dir` is nested inside.
pub const BACKUPS_ROOT_DIR_NAME: &str = "sandbox-backups";
const SCHEDULER_TICK_SECS: u64 = 60;

pub const BACKUP_INTERVAL_KEY: &str = "backup_interval_minutes";
pub const DEFAULT_BACKUP_INTERVAL_MINUTES: i64 = 15;
/// Gates only `run_scheduler`'s periodic ticking — manual backups and the
/// pre-stop/pre-delete consent backup are unaffected either way. Absent
/// (e.g. an existing install that predates this setting) means enabled,
/// matching the scheduler's behavior before this toggle existed.
pub const AUTO_BACKUP_ENABLED_KEY: &str = "auto_backup_enabled";
pub const BACKUP_KEEP_COUNT_KEY: &str = "backup_keep_count";
pub const MIN_BACKUP_KEEP_COUNT: i64 = 1;
pub const MAX_BACKUP_KEEP_COUNT: i64 = 10;

/// Which halves of a sandbox a backup (or restore) covers. `Claude`/`Codex`
/// both mean "this sandbox's own agent home directory" — a sandbox only
/// ever has one agent, so requesting the wrong one for a given sandbox is
/// rejected by `backup_sandbox`/`restore::restore_backup` rather than
/// silently doing nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupScope {
  Claude,
  Codex,
  Git,
  All,
}

impl BackupScope {
  pub fn parse(raw: &str) -> std::result::Result<Self, String> {
    match raw {
      "claude" => Ok(Self::Claude),
      "codex" => Ok(Self::Codex),
      "git" => Ok(Self::Git),
      "all" => Ok(Self::All),
      other => Err(format!("invalid backup scope: {other}")),
    }
  }

  pub fn wants_agent_home(self) -> bool {
    matches!(self, Self::Claude | Self::Codex | Self::All)
  }

  pub fn wants_git(self) -> bool {
    matches!(self, Self::Git | Self::All)
  }

  /// The agent id an explicit `Claude`/`Codex` scope names, or `None` for
  /// `Git`/`All` (which don't pin a specific agent — `All` always means
  /// "whichever agent this sandbox actually is").
  pub fn named_agent(self) -> Option<&'static str> {
    match self {
      Self::Claude => Some("claude"),
      Self::Codex => Some("codex"),
      Self::Git | Self::All => None,
    }
  }
}

/// Which sandboxes currently have a backup copy in flight, keyed by
/// sandbox id — managed as `Mutex<BackupState>` app state, same pattern as
/// `sbx::HostMonitor`/`sbx::SandboxMonitor`. Lets the frontend show a
/// progress indicator and disable Stop/Delete while a copy is running.
#[derive(Default)]
pub struct BackupState {
  active: HashMap<String, ActiveBackup>,
  cancels: HashMap<String, tokio::sync::watch::Sender<bool>>,
}

impl BackupState {
  fn cancel(&self, sandbox_id: &str) -> bool {
    self.cancels.get(sandbox_id).is_some_and(|tx| tx.send(true).is_ok())
  }
}

pub const BACKUP_CANCELLED: &str = "backup cancelled";

/// Asks the running backup of `sandbox_id` to stop. `false` when none is running.
pub fn cancel_backup(app: &AppHandle, sandbox_id: &str) -> bool {
  let Some(state) = app.try_state::<Mutex<BackupState>>() else {
    return false;
  };
  let Ok(state) = state.lock() else {
    return false;
  };
  state.cancel(sandbox_id)
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
        state.cancels.remove(&self.sandbox_id);
      }
    }
  }
}

/// Copies `sandbox_id`'s agent home directory and (if it has a git
/// workspace) `.git` directory to a fresh host folder, records the backup,
/// and prunes anything beyond the newest `keep_count`. A
/// missing `.git` (or a failed copy of either half) doesn't cancel the
/// other half — the two must stay independently importable. Every call
/// site treats this as best-effort and only logs the error.
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
  let agent_kit = crate::agents::get(&sandbox.agent).ok_or_else(|| format!("unknown agent: {}", sandbox.agent))?;
  if let Some(requested) = scope.named_agent() {
    if requested != sandbox.agent {
      return Err(format!("this sandbox's agent is \"{}\", not \"{requested}\"", sandbox.agent));
    }
  }

  let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
  if let Some(state) = app.try_state::<Mutex<BackupState>>() {
    if let Ok(mut state) = state.lock() {
      state.active.insert(
        sandbox_id.to_string(),
        ActiveBackup { sandbox_id: sandbox_id.to_string(), trigger: trigger.to_string(), started_at: now_millis() },
      );
      state.cancels.insert(sandbox_id.to_string(), cancel_tx);
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

  let agent_dir = root.join(agent_kit.id);
  let has_agent_home = if scope.wants_agent_home() {
    try_copy(app, &name, agent_kit.home_dir, &agent_dir, &cancel_rx).await
  } else {
    false
  };
  let (has_claude, has_codex) = match agent_kit.id {
    "codex" => (false, has_agent_home),
    _ => (has_agent_home, false),
  };

  let workspace = crate::sbx::workspace_path(app, &name).await.map_err(|e| e.to_string())?;
  let has_git = if scope.wants_git() {
    match &workspace {
      Some(ws) => try_copy(app, &name, &format!("{ws}/.git"), &root.join("git"), &cancel_rx).await,
      None => false,
    }
  } else {
    false
  };

  if *cancel_rx.borrow() {
    if let Err(e) = std::fs::remove_dir_all(&root) {
      log::warn!("backup_sandbox: failed to remove cancelled backup {}: {e}", root.display());
    }
    return Err(BACKUP_CANCELLED.to_string());
  }

  if !has_agent_home && !has_git {
    let _ = std::fs::remove_dir_all(&root);
    return Err("backup failed: nothing could be copied".to_string());
  }
  let mut warnings = Vec::new();
  if scope.wants_agent_home() && !has_agent_home {
    warnings.push(format!(".{} copy failed", agent_kit.id));
  }
  if scope.wants_git() && workspace.is_some() && !has_git {
    warnings.push(".git copy failed".to_string());
  }

  let agent_basename = agent_kit.home_dir.rsplit('/').next().unwrap_or("");
  let plan_file_count = count_plan_files(&agent_dir, agent_basename);

  let row = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let inserted = backups::insert(
      &conn,
      sandbox_id,
      Some(&sandbox_label),
      trigger,
      &root.to_string_lossy(),
      has_claude,
      has_codex,
      has_git,
      sandbox.base_branch.as_deref(),
      sandbox.current_branch.as_deref(),
      &sandbox.branches,
      plan_file_count,
      &warnings,
    )
    .map_err(|e| e.to_string())?;
    // Best-effort: reflects this snapshot's root (containing <agent>/ and
    // git/) as the sandbox's most recent backup, regardless of trigger —
    // a failure here shouldn't undo an otherwise-successful backup.
    if let Err(e) = sandboxes::record_backup(&conn, sandbox_id, &root.to_string_lossy(), inserted.created_at) {
      log::warn!("backup_sandbox: failed to update last_backup fields for {sandbox_id}: {e}");
    }
    inserted
  };

  prune_old_backups(pool, sandbox_id, keep_count(pool));

  Ok(row)
}

async fn try_copy(
  app: &AppHandle,
  name: &str,
  remote_path: &str,
  host_dest: &std::path::Path,
  cancel: &crate::sbx::CancelRx,
) -> bool {
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
  match crate::sbx::cp_from_sandbox(app, name, remote_path, &host_dest.to_string_lossy(), Some(cancel.clone())).await {
    Ok(()) => true,
    Err(e) => {
      log::warn!("backup_sandbox: failed to copy {remote_path} from {name}: {e}");
      false
    }
  }
}

/// `<agent>/plans` is nested inside the just-copied agent home directory,
/// so counting `.md` files here needs no extra remote copy. Checks both
/// `agent_dir/plans` and `agent_dir/<agent_basename>/plans` (e.g.
/// `.claude`/`.codex`) since `sbx cp`'s exact nesting behavior is
/// unverified (see `sbx::cp_from_sandbox`).
fn count_plan_files(agent_dir: &std::path::Path, agent_basename: &str) -> i64 {
  let candidates = [agent_dir.join("plans"), agent_dir.join(agent_basename).join("plans")];
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

fn prune_old_backups(pool: &DbPool, sandbox_id: &str, keep: i64) {
  let conn = match pool.get() {
    Ok(conn) => conn,
    Err(e) => {
      log::warn!("prune_old_backups: failed to get db connection: {e}");
      return;
    }
  };
  let victims = match backups::beyond_limit(&conn, sandbox_id, keep) {
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

/// Applies a lowered keep count right away instead of waiting for the next backup.
pub fn prune_all(pool: &DbPool) {
  let keep = keep_count(pool);
  let ids = pool.get().ok().and_then(|conn| backups::sandbox_ids(&conn).ok()).unwrap_or_default();
  for id in ids {
    prune_old_backups(pool, &id, keep);
  }
}

fn read_i64_setting(pool: &DbPool, key: &str) -> Option<i64> {
  let conn = pool.get().ok()?;
  settings::get(&conn, key).ok().flatten()?.parse().ok()
}

pub fn keep_count(pool: &DbPool) -> i64 {
  read_i64_setting(pool, BACKUP_KEEP_COUNT_KEY)
    .unwrap_or(MAX_BACKUP_KEEP_COUNT)
    .clamp(MIN_BACKUP_KEEP_COUNT, MAX_BACKUP_KEEP_COUNT)
}

pub fn is_auto_backup_enabled(pool: &DbPool) -> bool {
  let Ok(conn) = pool.get() else { return true };
  match settings::get(&conn, AUTO_BACKUP_ENABLED_KEY) {
    Ok(Some(raw)) => raw != "false",
    _ => true,
  }
}

/// A sandbox is due once its interval has passed since the later of its
/// last backup and the scheduler's last attempt (so a failing sandbox is
/// retried once per interval, not every tick).
fn is_due(now: i64, last_backup: i64, last_attempt: Option<i64>, interval_minutes: i64) -> bool {
  let last = last_attempt.map_or(last_backup, |attempt| attempt.max(last_backup));
  now - last >= interval_minutes * 60_000
}

/// Ticks every `SCHEDULER_TICK_SECS`, re-reading the settings each time so
/// a change takes effect on the next tick with no restart. Each running
/// sandbox with `backup_enabled` is backed up on its own interval (its
/// override, else the global one), measured from its `last_backup_at`
/// (or `created_at`), which survives restarts without a burst on launch.
pub async fn run_scheduler(app: AppHandle, pool: DbPool) {
  let mut last_attempt: HashMap<String, i64> = HashMap::new();
  loop {
    tokio::time::sleep(std::time::Duration::from_secs(SCHEDULER_TICK_SECS)).await;

    if !is_auto_backup_enabled(&pool) {
      continue;
    }

    let global_interval = read_i64_setting(&pool, BACKUP_INTERVAL_KEY).unwrap_or(DEFAULT_BACKUP_INTERVAL_MINUTES);

    let running_sandboxes = {
      let conn = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
          log::warn!("run_scheduler: failed to get db connection: {e}");
          continue;
        }
      };
      match sandboxes::list(&conn) {
        Ok(list) => list.into_iter().filter(|s| s.status == "running" && s.backup_enabled).collect::<Vec<_>>(),
        Err(e) => {
          log::warn!("run_scheduler: failed to list sandboxes: {e}");
          continue;
        }
      }
    };

    for sandbox in running_sandboxes {
      let interval = sandbox.backup_interval_minutes.unwrap_or(global_interval);
      let now = now_millis();
      let last_backup = sandbox.last_backup_at.unwrap_or(sandbox.created_at);
      if !is_due(now, last_backup, last_attempt.get(&sandbox.id).copied(), interval) {
        continue;
      }
      last_attempt.insert(sandbox.id.clone(), now);
      if let Err(e) = backup_sandbox(&app, &pool, &sandbox.id, "scheduled", BackupScope::All).await {
        if e == BACKUP_CANCELLED {
          continue;
        }
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
  fn is_due_uses_latest_of_backup_and_attempt() {
    let min = 60_000;
    assert!(!is_due(10 * min, 5 * min, None, 15));
    assert!(is_due(20 * min, 5 * min, None, 15));
    assert!(!is_due(20 * min, 5 * min, Some(10 * min), 15));
    assert!(is_due(25 * min, 5 * min, Some(10 * min), 15));
  }

  #[test]
  fn cancel_signals_only_a_running_backup() {
    let mut state = BackupState::default();
    assert!(!state.cancel("missing"));

    let (tx, rx) = tokio::sync::watch::channel(false);
    state.cancels.insert("sbx-1".to_string(), tx);
    assert!(state.cancel("sbx-1"));
    assert!(*rx.borrow());
  }

  #[test]
  fn parses_valid_scopes() {
    assert_eq!(BackupScope::parse("claude"), Ok(BackupScope::Claude));
    assert_eq!(BackupScope::parse("codex"), Ok(BackupScope::Codex));
    assert_eq!(BackupScope::parse("git"), Ok(BackupScope::Git));
    assert_eq!(BackupScope::parse("all"), Ok(BackupScope::All));
    assert!(BackupScope::parse("everything").is_err());
  }

  #[test]
  fn scope_gates_which_halves_are_wanted() {
    assert!(BackupScope::Claude.wants_agent_home());
    assert!(!BackupScope::Claude.wants_git());
    assert!(BackupScope::Codex.wants_agent_home());
    assert!(!BackupScope::Codex.wants_git());
    assert!(BackupScope::Git.wants_git());
    assert!(!BackupScope::Git.wants_agent_home());
    assert!(BackupScope::All.wants_agent_home());
    assert!(BackupScope::All.wants_git());
  }

  #[test]
  fn named_agent_only_set_for_claude_and_codex() {
    assert_eq!(BackupScope::Claude.named_agent(), Some("claude"));
    assert_eq!(BackupScope::Codex.named_agent(), Some("codex"));
    assert_eq!(BackupScope::Git.named_agent(), None);
    assert_eq!(BackupScope::All.named_agent(), None);
  }

  #[test]
  fn count_plan_files_returns_zero_when_missing() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    assert_eq!(count_plan_files(&dir, ".claude"), 0);
  }

  #[test]
  fn count_plan_files_counts_only_markdown() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    let plans = dir.join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    std::fs::write(plans.join("a.md"), "plan a").unwrap();
    std::fs::write(plans.join("b.md"), "plan b").unwrap();
    std::fs::write(plans.join("notes.txt"), "not a plan").unwrap();

    assert_eq!(count_plan_files(&dir, ".claude"), 2);

    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn count_plan_files_checks_nested_agent_dir() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    let plans = dir.join(".claude").join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    std::fs::write(plans.join("a.md"), "plan a").unwrap();

    assert_eq!(count_plan_files(&dir, ".claude"), 1);

    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn count_plan_files_checks_nested_codex_dir() {
    let dir = std::env::temp_dir().join(format!("overnight-backup-test-{}", crate::db::models::new_id()));
    let plans = dir.join(".codex").join("plans");
    std::fs::create_dir_all(&plans).unwrap();
    std::fs::write(plans.join("a.md"), "plan a").unwrap();

    assert_eq!(count_plan_files(&dir, ".codex"), 1);

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
    let sandbox = crate::db::sandboxes::create(&conn, &project.id, "mount", None, None, "default", None, "claude").unwrap();

    let dirs: Vec<_> = (0..12)
      .map(|i| std::env::temp_dir().join(format!("overnight-prune-test-{}-{i}", crate::db::models::new_id())))
      .collect();
    for dir in &dirs {
      std::fs::create_dir_all(dir).unwrap();
    }

    for (i, dir) in dirs.iter().enumerate() {
      let row =
        backups::insert(&conn, &sandbox.id, None, "scheduled", &dir.to_string_lossy(), true, false, true, None, None, &[], 0, &[]).unwrap();
      conn
        .execute("UPDATE sandbox_backups SET created_at = ?1 WHERE id = ?2", rusqlite::params![i as i64, row.id])
        .unwrap();
    }
    drop(conn);

    prune_old_backups(&pool, &sandbox.id, 10);

    let conn = pool.get().unwrap();
    assert_eq!(backups::list_all(&conn).unwrap().len(), 10);
    assert!(!dirs[0].exists());
    assert!(!dirs[1].exists());
    assert!(dirs[2].exists());
    drop(conn);

    prune_old_backups(&pool, &sandbox.id, 3);
    let conn = pool.get().unwrap();
    assert_eq!(backups::list_all(&conn).unwrap().len(), 3);
    assert!(!dirs[8].exists());
    assert!(dirs[9].exists());

    for dir in &dirs[9..] {
      std::fs::remove_dir_all(dir).ok();
    }
    drop(conn);
    drop(pool);
    std::fs::remove_file(&db_path).ok();
  }

  #[test]
  fn keep_count_defaults_to_ten_and_clamps() {
    let (pool, db_path) = test_pool();
    assert_eq!(keep_count(&pool), 10);
    for (raw, expected) in [("3", 3), ("0", 1), ("99", 10)] {
      settings::set(&pool.get().unwrap(), BACKUP_KEEP_COUNT_KEY, raw).unwrap();
      assert_eq!(keep_count(&pool), expected);
    }
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
