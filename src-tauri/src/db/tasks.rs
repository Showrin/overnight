use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Task};

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
  Ok(Task {
    id: row.get("id")?,
    title: row.get("title")?,
    description: row.get("description")?,
    jira_key: row.get("jira_key")?,
    status: row.get("status")?,
    project_path: row.get("project_path")?,
    metadata: row.get("metadata")?,
    created_at: row.get("created_at")?,
    updated_at: row.get("updated_at")?,
  })
}

pub fn create(
  conn: &Connection,
  title: &str,
  description: Option<&str>,
  jira_key: Option<&str>,
  status: &str,
  project_path: Option<&str>,
) -> Result<Task> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO tasks (id, title, description, jira_key, status, project_path, metadata, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
    params![id, title, description, jira_key, status, project_path, now],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Task> {
  conn
    .query_row("SELECT * FROM tasks WHERE id = ?1", params![id], row_to_task)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Task>> {
  let mut stmt = conn.prepare("SELECT * FROM tasks ORDER BY updated_at DESC")?;
  let rows = stmt.query_map([], row_to_task)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn update_status(conn: &Connection, id: &str, status: &str) -> Result<Task> {
  let now = now_millis();
  let changed = conn.execute(
    "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
    params![status, now, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  let changed = conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  #[test]
  fn create_get_list_update_delete() {
    let conn = test_conn();

    let task = create(&conn, "Write tests", None, Some("OVN-16"), "todo", None).unwrap();
    assert_eq!(task.title, "Write tests");
    assert_eq!(task.jira_key.as_deref(), Some("OVN-16"));

    let fetched = get(&conn, &task.id).unwrap();
    assert_eq!(fetched.id, task.id);

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);

    let updated = update_status(&conn, &task.id, "done").unwrap();
    assert_eq!(updated.status, "done");
    assert!(updated.updated_at >= task.updated_at);

    delete(&conn, &task.id).unwrap();
    assert!(matches!(get(&conn, &task.id), Err(Error::NotFound)));
  }

  #[test]
  fn duplicate_jira_key_rejected() {
    let conn = test_conn();
    create(&conn, "Task A", None, Some("OVN-1"), "todo", None).unwrap();
    let result = create(&conn, "Task B", None, Some("OVN-1"), "todo", None);
    assert!(result.is_err());
  }

  #[test]
  fn missing_id_operations_return_not_found() {
    let conn = test_conn();
    assert!(matches!(get(&conn, "missing"), Err(Error::NotFound)));
    assert!(matches!(
      update_status(&conn, "missing", "done"),
      Err(Error::NotFound)
    ));
    assert!(matches!(delete(&conn, "missing"), Err(Error::NotFound)));
  }
}
