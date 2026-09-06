use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Sandbox, WorktreeInfo};

fn row_to_sandbox(row: &rusqlite::Row) -> rusqlite::Result<Sandbox> {
  let branches_raw: String = row.get("branches")?;
  let worktrees_raw: String = row.get("worktrees")?;
  Ok(Sandbox {
    id: row.get("id")?,
    project_id: row.get("project_id")?,
    name: row.get("name")?,
    mode: row.get("mode")?,
    permission_mode: row.get("permission_mode")?,
    status: row.get("status")?,
    sbx_name: row.get("sbx_name")?,
    folder_path: row.get("folder_path")?,
    host_port: row.get("host_port")?,
    created_at: row.get("created_at")?,
    stopped_at: row.get("stopped_at")?,
    network_preset_override: row.get("network_preset_override")?,
    last_backup_at: row.get("last_backup_at")?,
    last_backup_path: row.get("last_backup_path")?,
    base_branch: row.get("base_branch")?,
    current_branch: row.get("current_branch")?,
    branches: serde_json::from_str(&branches_raw).unwrap_or_default(),
    worktrees: serde_json::from_str(&worktrees_raw).unwrap_or_default(),
    branch_snapshot_at: row.get("branch_snapshot_at")?,
  })
}

pub fn create(
  conn: &Connection,
  project_id: &str,
  mode: &str,
  folder_path: Option<&str>,
  name: Option<&str>,
  permission_mode: &str,
  base_branch: Option<&str>,
) -> Result<Sandbox> {
  let id = new_id();
  let now = now_millis();
  let result = conn.execute(
    "INSERT INTO sandboxes (id, project_id, mode, status, folder_path, name, permission_mode, created_at, base_branch)
     VALUES (?1, ?2, ?3, 'starting', ?4, ?5, ?6, ?7, ?8)",
    params![id, project_id, mode, folder_path, name, permission_mode, now, base_branch],
  );
  // ux_sandboxes_active_mount_per_project and ux_sandboxes_one_starting_per_project
  // close the race between callers' pre-insert checks and the insert itself.
  // Both are partial unique indexes on project_id, so a violation doesn't say
  // which one fired — inspect current rows to pick the right error message.
  if let Err(e) = result {
    return Err(if e.sqlite_extended_error_code() == Some(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE) {
      let existing = list_for_project(conn, project_id)?;
      if existing.iter().any(|s| s.mode == "mount" && matches!(s.status.as_str(), "starting" | "running")) {
        Error::DuplicateMountSandbox
      } else {
        Error::SandboxAlreadyStarting
      }
    } else {
      Error::Sqlite(e)
    });
  }
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Sandbox> {
  conn
    .query_row("SELECT * FROM sandboxes WHERE id = ?1", params![id], row_to_sandbox)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Sandbox>> {
  let mut stmt = conn.prepare("SELECT * FROM sandboxes ORDER BY created_at DESC")?;
  let rows = stmt.query_map([], row_to_sandbox)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_for_project(conn: &Connection, project_id: &str) -> Result<Vec<Sandbox>> {
  let mut stmt = conn.prepare("SELECT * FROM sandboxes WHERE project_id = ?1 ORDER BY created_at DESC")?;
  let rows = stmt.query_map(params![project_id], row_to_sandbox)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Looks up a sandbox row by its `sbx` CLI identity rather than our own
/// `id` — the orphan-adoption check's definition of "already known" is "a
/// row with this `sbx_name` exists", not any particular row id.
pub fn find_by_sbx_name(conn: &Connection, sbx_name: &str) -> Result<Option<Sandbox>> {
  match conn.query_row("SELECT * FROM sandboxes WHERE sbx_name = ?1", params![sbx_name], row_to_sandbox) {
    Ok(sandbox) => Ok(Some(sandbox)),
    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
    Err(e) => Err(Error::Sqlite(e)),
  }
}

/// Updates lifecycle status. `sbx_name`/`host_port` are only applied
/// when `Some` (e.g. once `sbx create` succeeds), leaving prior values
/// intact otherwise. `stopped_at` is set automatically when `status` is
/// "stopped" and cleared for any other status (e.g. resuming via "running").
pub fn update_status(
  conn: &Connection,
  id: &str,
  status: &str,
  sbx_name: Option<&str>,
  host_port: Option<i64>,
) -> Result<Sandbox> {
  let stopped_at = if status == "stopped" { Some(now_millis()) } else { None };
  let changed = conn.execute(
    "UPDATE sandboxes
     SET status = ?1, sbx_name = COALESCE(?2, sbx_name), host_port = COALESCE(?3, host_port), stopped_at = ?4
     WHERE id = ?5",
    params![status, sbx_name, host_port, stopped_at, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

/// `sbx policy reset` restarts sbx's network daemon, which stops every
/// sandbox on the machine as a side effect (confirmed against a real `sbx`
/// install) — with no per-sandbox event to react to. Called right after a
/// successful reset so the DB doesn't keep showing sandboxes as running
/// when they've actually all been stopped out from under the app.
pub fn stop_all_for_policy_reset(conn: &Connection) -> Result<usize> {
  let stopped_at = now_millis();
  Ok(conn.execute(
    "UPDATE sandboxes SET status = 'stopped', stopped_at = ?1 WHERE status IN ('running', 'starting', 'stopping')",
    params![stopped_at],
  )?)
}

/// Persists (or clears, when `preset` is `None`) this sandbox's network
/// policy preset override. Callers apply the actual sbx-side effect
/// (`sbx policy allow/deny/rm --sandbox <name> "**"`) separately —  this
/// only records the choice, mirroring how `permission_mode` is snapshotted
/// without this module knowing anything about Claude Code itself.
pub fn set_network_preset_override(conn: &Connection, id: &str, preset: Option<&str>) -> Result<Sandbox> {
  let changed = conn.execute(
    "UPDATE sandboxes SET network_preset_override = ?1 WHERE id = ?2",
    params![preset, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

/// Records a successful `sbx cp` backup of this sandbox's `~/.claude`
/// directory — called once the copy itself has already succeeded, mirroring
/// `set_network_preset_override`'s "apply the sbx-side effect elsewhere,
/// persist the record here" split.
pub fn record_backup(conn: &Connection, id: &str, path: &str, at: i64) -> Result<Sandbox> {
  let changed = conn.execute(
    "UPDATE sandboxes SET last_backup_at = ?1, last_backup_path = ?2 WHERE id = ?3",
    params![at, path, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

/// Persists one "branch snapshot" — current branch, local branch list, and
/// worktree list, all captured together in a single `sbx exec` round trip
/// (see `sbx::read_branch_snapshot`) — as of `at`. `branches`/`worktrees`
/// are JSON-serialized into their TEXT columns, mirroring
/// `Project.extra_clone_paths`'s storage pattern.
pub fn record_branch_snapshot(
  conn: &Connection,
  id: &str,
  current_branch: Option<&str>,
  branches: &[String],
  worktrees: &[WorktreeInfo],
  at: i64,
) -> Result<Sandbox> {
  let branches_json = serde_json::to_string(branches).unwrap_or_else(|_| "[]".to_string());
  let worktrees_json = serde_json::to_string(worktrees).unwrap_or_else(|_| "[]".to_string());
  let changed = conn.execute(
    "UPDATE sandboxes
     SET current_branch = ?1, branches = ?2, worktrees = ?3, branch_snapshot_at = ?4
     WHERE id = ?5",
    params![current_branch, branches_json, worktrees_json, at, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  let changed = conn.execute("DELETE FROM sandboxes WHERE id = ?1", params![id])?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  fn make_project(conn: &Connection) -> String {
    crate::db::projects::create(conn, "Overnight", "/repo/overnight", None, None).unwrap().id
  }

  #[test]
  fn create_get_list_update_delete() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let sandbox = create(&conn, &project_id, "mount", None, None, "default", None).unwrap();
    assert_eq!(sandbox.status, "starting");
    assert_eq!(sandbox.mode, "mount");
    assert_eq!(sandbox.sbx_name, None);
    assert_eq!(sandbox.name, None);
    assert_eq!(sandbox.permission_mode, "default");

    let fetched = get(&conn, &sandbox.id).unwrap();
    assert_eq!(fetched.id, sandbox.id);

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);

    let for_project = list_for_project(&conn, &project_id).unwrap();
    assert_eq!(for_project.len(), 1);

    let running = update_status(&conn, &sandbox.id, "running", Some("container123"), Some(4173)).unwrap();
    assert_eq!(running.status, "running");
    assert_eq!(running.sbx_name.as_deref(), Some("container123"));
    assert_eq!(running.host_port, Some(4173));
    assert_eq!(running.stopped_at, None);

    let stopped = update_status(&conn, &sandbox.id, "stopped", None, None).unwrap();
    assert_eq!(stopped.status, "stopped");
    // sbx_name/host_port preserved even though None was passed this time.
    assert_eq!(stopped.sbx_name.as_deref(), Some("container123"));
    assert!(stopped.stopped_at.is_some());

    delete(&conn, &sandbox.id).unwrap();
    assert!(matches!(get(&conn, &sandbox.id), Err(Error::NotFound)));
  }

  #[test]
  fn missing_id_operations_return_not_found() {
    let conn = test_conn();
    assert!(matches!(get(&conn, "missing"), Err(Error::NotFound)));
    assert!(matches!(
      update_status(&conn, "missing", "running", None, None),
      Err(Error::NotFound)
    ));
    assert!(matches!(delete(&conn, "missing"), Err(Error::NotFound)));
  }

  #[test]
  fn stop_all_for_policy_reset_stops_only_unsettled_sandboxes() {
    let conn = test_conn();
    // Separate projects, since a partial unique index allows only one
    // "starting" sandbox per project at a time.
    let running = create(&conn, &make_project(&conn), "clone", None, None, "default", None).unwrap();
    update_status(&conn, &running.id, "running", Some("sbx-a"), None).unwrap();
    let starting = create(&conn, &make_project(&conn), "clone", None, None, "default", None).unwrap();
    let already_stopped = create(&conn, &make_project(&conn), "clone", None, None, "default", None).unwrap();
    update_status(&conn, &already_stopped.id, "stopped", Some("sbx-c"), None).unwrap();

    let affected = stop_all_for_policy_reset(&conn).unwrap();
    assert_eq!(affected, 2); // running + starting, not the already-stopped one

    assert_eq!(get(&conn, &running.id).unwrap().status, "stopped");
    assert_eq!(get(&conn, &starting.id).unwrap().status, "stopped");
    assert!(get(&conn, &running.id).unwrap().stopped_at.is_some());
    // sbx_name is preserved — the sandbox itself isn't removed, just stopped.
    assert_eq!(get(&conn, &running.id).unwrap().sbx_name.as_deref(), Some("sbx-a"));
  }

  #[test]
  fn stores_and_returns_optional_name() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let named = create(&conn, &project_id, "clone", None, Some("staging"), "default", None).unwrap();
    assert_eq!(named.name.as_deref(), Some("staging"));
    assert_eq!(get(&conn, &named.id).unwrap().name.as_deref(), Some("staging"));
    update_status(&conn, &named.id, "running", Some("sbx-1"), None).unwrap();

    let unnamed = create(&conn, &project_id, "clone", None, None, "default", None).unwrap();
    assert_eq!(unnamed.name, None);
  }

  #[test]
  fn stores_and_returns_permission_mode() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let sandbox = create(&conn, &project_id, "clone", None, None, "bypassPermissions", None).unwrap();
    assert_eq!(sandbox.permission_mode, "bypassPermissions");
    assert_eq!(get(&conn, &sandbox.id).unwrap().permission_mode, "bypassPermissions");
  }

  #[test]
  fn concurrent_mount_create_is_rejected_by_db_constraint() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let mount = create(&conn, &project_id, "mount", None, None, "default", None).unwrap();
    let second = create(&conn, &project_id, "mount", None, None, "default", None);
    assert!(matches!(second, Err(Error::DuplicateMountSandbox)));

    // Still rejected once the first is "running", not just "starting".
    update_status(&conn, &mount.id, "running", Some("sbx-1"), None).unwrap();
    let second_after_running = create(&conn, &project_id, "mount", None, None, "default", None);
    assert!(matches!(second_after_running, Err(Error::DuplicateMountSandbox)));

    // Other projects are unaffected by the partial index.
    let other_project_id = make_project(&conn);
    create(&conn, &other_project_id, "mount", None, None, "default", None).unwrap();
  }

  #[test]
  fn concurrent_starting_create_is_rejected_regardless_of_mode() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let first = create(&conn, &project_id, "clone", None, None, "default", None).unwrap();
    let second = create(&conn, &project_id, "clone", None, None, "default", None);
    assert!(matches!(second, Err(Error::SandboxAlreadyStarting)));

    // Once the first finishes starting, clone mode still allows several
    // sandboxes running at once for the same project.
    update_status(&conn, &first.id, "running", Some("sbx-1"), None).unwrap();
    create(&conn, &project_id, "clone", None, None, "default", None).unwrap();
  }

  #[test]
  fn sets_and_clears_network_preset_override() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox = create(&conn, &project_id, "mount", None, None, "default", None).unwrap();
    assert_eq!(sandbox.network_preset_override, None);

    let overridden = set_network_preset_override(&conn, &sandbox.id, Some("open")).unwrap();
    assert_eq!(overridden.network_preset_override.as_deref(), Some("open"));
    assert_eq!(get(&conn, &sandbox.id).unwrap().network_preset_override.as_deref(), Some("open"));

    let cleared = set_network_preset_override(&conn, &sandbox.id, None).unwrap();
    assert_eq!(cleared.network_preset_override, None);
  }

  #[test]
  fn set_network_preset_override_missing_id_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(set_network_preset_override(&conn, "missing", Some("open")), Err(Error::NotFound)));
  }

  #[test]
  fn stores_base_branch_and_defaults_branch_snapshot_fields() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let sandbox = create(&conn, &project_id, "mount", None, None, "default", Some("feature/x")).unwrap();
    assert_eq!(sandbox.base_branch.as_deref(), Some("feature/x"));
    assert_eq!(sandbox.current_branch, None);
    assert_eq!(sandbox.branches, Vec::<String>::new());
    assert_eq!(sandbox.worktrees, Vec::new());
    assert_eq!(sandbox.branch_snapshot_at, None);

    let other_project_id = make_project(&conn);
    let no_base_branch = create(&conn, &other_project_id, "clone", None, None, "default", None).unwrap();
    assert_eq!(no_base_branch.base_branch, None);
  }

  #[test]
  fn records_and_returns_branch_snapshot() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox = create(&conn, &project_id, "mount", None, None, "default", Some("main")).unwrap();

    let worktrees =
      vec![WorktreeInfo { path: "/home/agent/proj".to_string(), branch: Some("main".to_string()), head_sha: "abc123".to_string() }];
    let branches = vec!["main".to_string(), "feature".to_string()];
    let updated = record_branch_snapshot(&conn, &sandbox.id, Some("feature"), &branches, &worktrees, 1700000000000).unwrap();

    assert_eq!(updated.current_branch.as_deref(), Some("feature"));
    assert_eq!(updated.branches, branches);
    assert_eq!(updated.worktrees, worktrees);
    assert_eq!(updated.branch_snapshot_at, Some(1700000000000));

    let fetched = get(&conn, &sandbox.id).unwrap();
    assert_eq!(fetched.branches, branches);
    assert_eq!(fetched.worktrees, worktrees);
  }

  #[test]
  fn record_branch_snapshot_missing_id_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(
      record_branch_snapshot(&conn, "missing", None, &[], &[], 0),
      Err(Error::NotFound)
    ));
  }

  #[test]
  fn records_backup() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox = create(&conn, &project_id, "mount", None, None, "default", None).unwrap();
    assert_eq!(sandbox.last_backup_at, None);
    assert_eq!(sandbox.last_backup_path, None);

    let backed_up = record_backup(&conn, &sandbox.id, "/data/claude-backups/my-sbx/1700000000000", 1700000000000).unwrap();
    assert_eq!(backed_up.last_backup_at, Some(1700000000000));
    assert_eq!(
      backed_up.last_backup_path.as_deref(),
      Some("/data/claude-backups/my-sbx/1700000000000")
    );
    assert_eq!(get(&conn, &sandbox.id).unwrap().last_backup_at, Some(1700000000000));
  }

  #[test]
  fn record_backup_missing_id_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(record_backup(&conn, "missing", "/some/path", 0), Err(Error::NotFound)));
  }

  #[test]
  fn finds_sandbox_by_sbx_name() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox = create(&conn, &project_id, "mount", None, None, "default", None).unwrap();
    update_status(&conn, &sandbox.id, "running", Some("my-sbx-name"), None).unwrap();

    let found = find_by_sbx_name(&conn, "my-sbx-name").unwrap();
    assert_eq!(found.map(|s| s.id), Some(sandbox.id));

    assert!(find_by_sbx_name(&conn, "no-such-name").unwrap().is_none());
  }

  #[test]
  fn deleting_project_cascades_to_sandboxes() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox =
      create(&conn, &project_id, "clone", Some("/data/sandboxes/abc"), Some("my sandbox"), "default", None).unwrap();

    crate::db::projects::delete(&conn, &project_id).unwrap();

    assert!(matches!(get(&conn, &sandbox.id), Err(Error::NotFound)));
  }
}
