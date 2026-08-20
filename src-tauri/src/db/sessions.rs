use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Session};

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<Session> {
  Ok(Session {
    id: row.get("id")?,
    task_id: row.get("task_id")?,
    agent_provider: row.get("agent_provider")?,
    provider_session_id: row.get("provider_session_id")?,
    parent_session_id: row.get("parent_session_id")?,
    mode: row.get("mode")?,
    status: row.get("status")?,
    started_at: row.get("started_at")?,
    ended_at: row.get("ended_at")?,
    transcript_path: row.get("transcript_path")?,
    plan_path: row.get("plan_path")?,
  })
}

pub fn create(
  conn: &Connection,
  task_id: &str,
  agent_provider: &str,
  mode: &str,
  parent_session_id: Option<&str>,
) -> Result<Session> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO sessions (id, task_id, agent_provider, provider_session_id, parent_session_id, mode, status, started_at, ended_at, transcript_path, plan_path)
     VALUES (?1, ?2, ?3, NULL, ?4, ?5, 'running', ?6, NULL, NULL, NULL)",
    params![id, task_id, agent_provider, parent_session_id, mode, now],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Session> {
  conn
    .query_row("SELECT * FROM sessions WHERE id = ?1", params![id], row_to_session)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list_for_task(conn: &Connection, task_id: &str) -> Result<Vec<Session>> {
  let mut stmt = conn.prepare("SELECT * FROM sessions WHERE task_id = ?1 ORDER BY started_at DESC")?;
  let rows = stmt.query_map(params![task_id], row_to_session)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[allow(dead_code)] // consumed by OVN-53's resume() once the AgentProvider trait lands
pub fn set_provider_session_id(conn: &Connection, id: &str, provider_session_id: &str) -> Result<Session> {
  let changed = conn.execute(
    "UPDATE sessions SET provider_session_id = ?1 WHERE id = ?2",
    params![provider_session_id, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn end(conn: &Connection, id: &str, status: &str) -> Result<Session> {
  let now = now_millis();
  let changed = conn.execute(
    "UPDATE sessions SET status = ?1, ended_at = ?2 WHERE id = ?3",
    params![status, now, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::tasks;

  fn make_task(conn: &Connection) -> String {
    tasks::create(conn, "Task", None, None, "todo", None).unwrap().id
  }

  #[test]
  fn create_get_list_end() {
    let conn = test_conn();
    let task_id = make_task(&conn);

    let session = create(&conn, &task_id, "claude_code", "plan", None).unwrap();
    assert_eq!(session.status, "running");
    assert!(session.provider_session_id.is_none());

    let fetched = get(&conn, &session.id).unwrap();
    assert_eq!(fetched.id, session.id);

    let list = list_for_task(&conn, &task_id).unwrap();
    assert_eq!(list.len(), 1);

    let with_provider_id = set_provider_session_id(&conn, &session.id, "cc-abc123").unwrap();
    assert_eq!(with_provider_id.provider_session_id.as_deref(), Some("cc-abc123"));

    let ended = end(&conn, &session.id, "completed").unwrap();
    assert_eq!(ended.status, "completed");
    assert!(ended.ended_at.is_some());
  }

  #[test]
  fn resume_creates_child_session() {
    let conn = test_conn();
    let task_id = make_task(&conn);
    let original = create(&conn, &task_id, "claude_code", "plan", None).unwrap();

    let resumed = create(&conn, &task_id, "claude_code", "plan", Some(&original.id)).unwrap();
    assert_eq!(resumed.parent_session_id.as_deref(), Some(original.id.as_str()));
  }

  #[test]
  fn deleting_task_cascades_to_sessions() {
    let conn = test_conn();
    let task_id = make_task(&conn);
    let session = create(&conn, &task_id, "claude_code", "plan", None).unwrap();

    tasks::delete(&conn, &task_id).unwrap();

    assert!(matches!(get(&conn, &session.id), Err(Error::NotFound)));
  }
}
