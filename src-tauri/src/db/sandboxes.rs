use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Sandbox};

fn row_to_sandbox(row: &rusqlite::Row) -> rusqlite::Result<Sandbox> {
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
  })
}

pub fn create(
  conn: &Connection,
  project_id: &str,
  mode: &str,
  folder_path: Option<&str>,
  name: Option<&str>,
  permission_mode: &str,
) -> Result<Sandbox> {
  let id = new_id();
  let now = now_millis();
  let result = conn.execute(
    "INSERT INTO sandboxes (id, project_id, mode, status, folder_path, name, permission_mode, created_at)
     VALUES (?1, ?2, ?3, 'starting', ?4, ?5, ?6, ?7)",
    params![id, project_id, mode, folder_path, name, permission_mode, now],
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

    let sandbox = create(&conn, &project_id, "mount", None, None, "default").unwrap();
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
  fn stores_and_returns_optional_name() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let named = create(&conn, &project_id, "clone", None, Some("staging"), "default").unwrap();
    assert_eq!(named.name.as_deref(), Some("staging"));
    assert_eq!(get(&conn, &named.id).unwrap().name.as_deref(), Some("staging"));
    update_status(&conn, &named.id, "running", Some("sbx-1"), None).unwrap();

    let unnamed = create(&conn, &project_id, "clone", None, None, "default").unwrap();
    assert_eq!(unnamed.name, None);
  }

  #[test]
  fn stores_and_returns_permission_mode() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let sandbox = create(&conn, &project_id, "clone", None, None, "bypassPermissions").unwrap();
    assert_eq!(sandbox.permission_mode, "bypassPermissions");
    assert_eq!(get(&conn, &sandbox.id).unwrap().permission_mode, "bypassPermissions");
  }

  #[test]
  fn concurrent_mount_create_is_rejected_by_db_constraint() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let mount = create(&conn, &project_id, "mount", None, None, "default").unwrap();
    let second = create(&conn, &project_id, "mount", None, None, "default");
    assert!(matches!(second, Err(Error::DuplicateMountSandbox)));

    // Still rejected once the first is "running", not just "starting".
    update_status(&conn, &mount.id, "running", Some("sbx-1"), None).unwrap();
    let second_after_running = create(&conn, &project_id, "mount", None, None, "default");
    assert!(matches!(second_after_running, Err(Error::DuplicateMountSandbox)));

    // Other projects are unaffected by the partial index.
    let other_project_id = make_project(&conn);
    create(&conn, &other_project_id, "mount", None, None, "default").unwrap();
  }

  #[test]
  fn concurrent_starting_create_is_rejected_regardless_of_mode() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let first = create(&conn, &project_id, "clone", None, None, "default").unwrap();
    let second = create(&conn, &project_id, "clone", None, None, "default");
    assert!(matches!(second, Err(Error::SandboxAlreadyStarting)));

    // Once the first finishes starting, clone mode still allows several
    // sandboxes running at once for the same project.
    update_status(&conn, &first.id, "running", Some("sbx-1"), None).unwrap();
    create(&conn, &project_id, "clone", None, None, "default").unwrap();
  }

  #[test]
  fn deleting_project_cascades_to_sandboxes() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox =
      create(&conn, &project_id, "clone", Some("/data/sandboxes/abc"), Some("my sandbox"), "default").unwrap();

    crate::db::projects::delete(&conn, &project_id).unwrap();

    assert!(matches!(get(&conn, &sandbox.id), Err(Error::NotFound)));
  }
}
