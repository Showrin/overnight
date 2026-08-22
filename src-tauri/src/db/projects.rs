use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Project};

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
  let extra_clone_paths_raw: String = row.get("extra_clone_paths")?;
  Ok(Project {
    id: row.get("id")?,
    name: row.get("name")?,
    repo_path: row.get("repo_path")?,
    plans_path: row.get("plans_path")?,
    dev_server_port: row.get("dev_server_port")?,
    extra_clone_paths: serde_json::from_str(&extra_clone_paths_raw).unwrap_or_default(),
    created_at: row.get("created_at")?,
    updated_at: row.get("updated_at")?,
  })
}

pub fn create(
  conn: &Connection,
  name: &str,
  repo_path: &str,
  plans_path: Option<&str>,
  dev_server_port: Option<i64>,
  extra_clone_paths: &[String],
) -> Result<Project> {
  let id = new_id();
  let now = now_millis();
  let extra_clone_paths_raw = serde_json::to_string(extra_clone_paths)?;
  conn.execute(
    "INSERT INTO projects (id, name, repo_path, plans_path, dev_server_port, extra_clone_paths, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
    params![id, name, repo_path, plans_path, dev_server_port, extra_clone_paths_raw, now],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Project> {
  conn
    .query_row("SELECT * FROM projects WHERE id = ?1", params![id], row_to_project)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Project>> {
  let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY updated_at DESC")?;
  let rows = stmt.query_map([], row_to_project)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn update(
  conn: &Connection,
  id: &str,
  name: &str,
  repo_path: &str,
  plans_path: Option<&str>,
  dev_server_port: Option<i64>,
  extra_clone_paths: &[String],
) -> Result<Project> {
  let now = now_millis();
  let extra_clone_paths_raw = serde_json::to_string(extra_clone_paths)?;
  let changed = conn.execute(
    "UPDATE projects
     SET name = ?1, repo_path = ?2, plans_path = ?3, dev_server_port = ?4,
         extra_clone_paths = ?5, updated_at = ?6
     WHERE id = ?7",
    params![name, repo_path, plans_path, dev_server_port, extra_clone_paths_raw, now, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  let changed = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  fn sample_paths() -> Vec<String> {
    vec!["*.env".to_string(), "docs/**".to_string()]
  }

  #[test]
  fn create_get_list_update_delete() {
    let conn = test_conn();

    let project = create(&conn, "Overnight", "/repo/overnight", None, Some(5173), &sample_paths()).unwrap();
    assert_eq!(project.name, "Overnight");
    assert_eq!(project.dev_server_port, Some(5173));
    assert_eq!(project.extra_clone_paths, sample_paths());

    let fetched = get(&conn, &project.id).unwrap();
    assert_eq!(fetched.id, project.id);

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);

    let updated = update(
      &conn,
      &project.id,
      "Overnight Renamed",
      "/repo/overnight",
      Some(".agent/plans"),
      Some(5174),
      &[],
    )
    .unwrap();
    assert_eq!(updated.name, "Overnight Renamed");
    assert_eq!(updated.plans_path.as_deref(), Some(".agent/plans"));
    assert_eq!(updated.dev_server_port, Some(5174));
    assert!(updated.extra_clone_paths.is_empty());
    assert!(updated.updated_at >= project.updated_at);

    delete(&conn, &project.id).unwrap();
    assert!(matches!(get(&conn, &project.id), Err(Error::NotFound)));
  }

  #[test]
  fn missing_id_operations_return_not_found() {
    let conn = test_conn();
    assert!(matches!(get(&conn, "missing"), Err(Error::NotFound)));
    assert!(matches!(
      update(&conn, "missing", "x", "/repo", None, None, &[]),
      Err(Error::NotFound)
    ));
    assert!(matches!(delete(&conn, "missing"), Err(Error::NotFound)));
  }

  #[test]
  fn deleting_project_nulls_task_project_id() {
    let conn = test_conn();
    let project = create(&conn, "Overnight", "/repo/overnight", None, None, &[]).unwrap();
    let task = crate::db::tasks::create(&conn, "Do work", None, None, "todo", None).unwrap();
    conn
      .execute(
        "UPDATE tasks SET project_id = ?1 WHERE id = ?2",
        params![project.id, task.id],
      )
      .unwrap();

    delete(&conn, &project.id).unwrap();

    let task = crate::db::tasks::get(&conn, &task.id).unwrap();
    assert_eq!(task.project_id, None);
  }
}
