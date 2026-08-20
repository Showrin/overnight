use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, Activity};

fn row_to_activity(row: &rusqlite::Row) -> rusqlite::Result<Activity> {
  Ok(Activity {
    id: row.get("id")?,
    session_id: row.get("session_id")?,
    occurred_at: row.get("occurred_at")?,
    event_type: row.get("event_type")?,
    payload: row.get("payload")?,
  })
}

pub fn append(conn: &Connection, session_id: &str, event_type: &str, payload: &str) -> Result<Activity> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO activity (id, session_id, occurred_at, event_type, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![id, session_id, now, event_type, payload],
  )?;
  let activity = conn.query_row("SELECT * FROM activity WHERE id = ?1", params![id], row_to_activity)?;
  Ok(activity)
}

pub fn list_for_session(conn: &Connection, session_id: &str) -> Result<Vec<Activity>> {
  let mut stmt =
    conn.prepare("SELECT * FROM activity WHERE session_id = ?1 ORDER BY occurred_at ASC")?;
  let rows = stmt.query_map(params![session_id], row_to_activity)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::{sessions, tasks};

  fn make_session(conn: &Connection) -> String {
    let task = tasks::create(conn, "Task", None, None, "todo", None).unwrap();
    sessions::create(conn, &task.id, "claude_code", "plan", None).unwrap().id
  }

  #[test]
  fn append_and_list_in_order() {
    let conn = test_conn();
    let session_id = make_session(&conn);

    append(&conn, &session_id, "message", r#"{"role":"user","text":"hi"}"#).unwrap();
    append(&conn, &session_id, "message", r#"{"role":"assistant","text":"hello"}"#).unwrap();

    let events = list_for_session(&conn, &session_id).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "message");
    assert!(events[0].occurred_at <= events[1].occurred_at);
  }

  #[test]
  fn deleting_session_cascades_to_activity() {
    let conn = test_conn();
    let session_id = make_session(&conn);
    append(&conn, &session_id, "message", "{}").unwrap();

    sessions::end(&conn, &session_id, "completed").unwrap();
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id]).unwrap();

    let events = list_for_session(&conn, &session_id).unwrap();
    assert!(events.is_empty());
  }
}
