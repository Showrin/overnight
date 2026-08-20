use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, ContainerMetric};

fn row_to_container_metric(row: &rusqlite::Row) -> rusqlite::Result<ContainerMetric> {
  Ok(ContainerMetric {
    id: row.get("id")?,
    session_id: row.get("session_id")?,
    captured_at: row.get("captured_at")?,
    cpu_percent: row.get("cpu_percent")?,
    memory_mb: row.get("memory_mb")?,
  })
}

pub fn record(conn: &Connection, session_id: &str, cpu_percent: f64, memory_mb: f64) -> Result<ContainerMetric> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO container_metrics (id, session_id, captured_at, cpu_percent, memory_mb) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![id, session_id, now, cpu_percent, memory_mb],
  )?;
  let metric = conn.query_row(
    "SELECT * FROM container_metrics WHERE id = ?1",
    params![id],
    row_to_container_metric,
  )?;
  Ok(metric)
}

pub fn list_for_session(conn: &Connection, session_id: &str) -> Result<Vec<ContainerMetric>> {
  let mut stmt =
    conn.prepare("SELECT * FROM container_metrics WHERE session_id = ?1 ORDER BY captured_at ASC")?;
  let rows = stmt.query_map(params![session_id], row_to_container_metric)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::{sessions, tasks};

  fn make_session(conn: &Connection) -> String {
    let task = tasks::create(conn, "Task", None, None, "todo", None).unwrap();
    sessions::create(conn, &task.id, "claude_code", "autonomous", None).unwrap().id
  }

  #[test]
  fn record_and_list() {
    let conn = test_conn();
    let session_id = make_session(&conn);

    record(&conn, &session_id, 12.5, 256.0).unwrap();
    record(&conn, &session_id, 40.0, 512.0).unwrap();

    let list = list_for_session(&conn, &session_id).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].cpu_percent, 40.0);
  }
}
