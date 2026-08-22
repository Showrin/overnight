use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Sandbox};

fn row_to_sandbox(row: &rusqlite::Row) -> rusqlite::Result<Sandbox> {
  Ok(Sandbox {
    id: row.get("id")?,
    project_id: row.get("project_id")?,
    mode: row.get("mode")?,
    status: row.get("status")?,
    container_id: row.get("container_id")?,
    folder_path: row.get("folder_path")?,
    host_port: row.get("host_port")?,
    created_at: row.get("created_at")?,
    stopped_at: row.get("stopped_at")?,
  })
}

pub fn create(conn: &Connection, project_id: &str, mode: &str, folder_path: Option<&str>) -> Result<Sandbox> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO sandboxes (id, project_id, mode, status, folder_path, created_at)
     VALUES (?1, ?2, ?3, 'starting', ?4, ?5)",
    params![id, project_id, mode, folder_path, now],
  )?;
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

/// Updates lifecycle status. `container_id`/`host_port` are only applied
/// when `Some` (e.g. once `docker run` returns an id), leaving prior values
/// intact otherwise. `stopped_at` is set automatically when `status` is
/// "stopped" and cleared for any other status (e.g. resuming via "running").
pub fn update_status(
  conn: &Connection,
  id: &str,
  status: &str,
  container_id: Option<&str>,
  host_port: Option<i64>,
) -> Result<Sandbox> {
  let stopped_at = if status == "stopped" { Some(now_millis()) } else { None };
  let changed = conn.execute(
    "UPDATE sandboxes
     SET status = ?1, container_id = COALESCE(?2, container_id), host_port = COALESCE(?3, host_port), stopped_at = ?4
     WHERE id = ?5",
    params![status, container_id, host_port, stopped_at, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn set_folder_path(conn: &Connection, id: &str, folder_path: &str) -> Result<Sandbox> {
  let changed = conn.execute(
    "UPDATE sandboxes SET folder_path = ?1 WHERE id = ?2",
    params![folder_path, id],
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
    crate::db::projects::create(conn, "Overnight", "/repo/overnight", None, None, &[]).unwrap().id
  }

  #[test]
  fn create_get_list_update_delete() {
    let conn = test_conn();
    let project_id = make_project(&conn);

    let sandbox = create(&conn, &project_id, "mount", None).unwrap();
    assert_eq!(sandbox.status, "starting");
    assert_eq!(sandbox.mode, "mount");
    assert_eq!(sandbox.container_id, None);

    let fetched = get(&conn, &sandbox.id).unwrap();
    assert_eq!(fetched.id, sandbox.id);

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);

    let for_project = list_for_project(&conn, &project_id).unwrap();
    assert_eq!(for_project.len(), 1);

    let running = update_status(&conn, &sandbox.id, "running", Some("container123"), Some(4173)).unwrap();
    assert_eq!(running.status, "running");
    assert_eq!(running.container_id.as_deref(), Some("container123"));
    assert_eq!(running.host_port, Some(4173));
    assert_eq!(running.stopped_at, None);

    let stopped = update_status(&conn, &sandbox.id, "stopped", None, None).unwrap();
    assert_eq!(stopped.status, "stopped");
    // container_id/host_port preserved even though None was passed this time.
    assert_eq!(stopped.container_id.as_deref(), Some("container123"));
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
  fn deleting_project_cascades_to_sandboxes() {
    let conn = test_conn();
    let project_id = make_project(&conn);
    let sandbox = create(&conn, &project_id, "clone", Some("/data/sandboxes/abc")).unwrap();

    crate::db::projects::delete(&conn, &project_id).unwrap();

    assert!(matches!(get(&conn, &sandbox.id), Err(Error::NotFound)));
  }
}
