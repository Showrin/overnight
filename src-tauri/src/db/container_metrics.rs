use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, ContainerMetric};

fn row_to_container_metric(row: &rusqlite::Row) -> rusqlite::Result<ContainerMetric> {
  Ok(ContainerMetric {
    id: row.get("id")?,
    session_id: row.get("session_id")?,
    sandbox_id: row.get("sandbox_id")?,
    captured_at: row.get("captured_at")?,
    cpu_percent: row.get("cpu_percent")?,
    memory_mb: row.get("memory_mb")?,
    network_rx_bytes: row.get("network_rx_bytes")?,
    network_tx_bytes: row.get("network_tx_bytes")?,
  })
}

pub fn record_for_session(
  conn: &Connection,
  session_id: &str,
  cpu_percent: f64,
  memory_mb: f64,
  network_rx_bytes: f64,
  network_tx_bytes: f64,
) -> Result<ContainerMetric> {
  record(conn, Some(session_id), None, cpu_percent, memory_mb, network_rx_bytes, network_tx_bytes)
}

pub fn record_for_sandbox(
  conn: &Connection,
  sandbox_id: &str,
  cpu_percent: f64,
  memory_mb: f64,
  network_rx_bytes: f64,
  network_tx_bytes: f64,
) -> Result<ContainerMetric> {
  record(conn, None, Some(sandbox_id), cpu_percent, memory_mb, network_rx_bytes, network_tx_bytes)
}

fn record(
  conn: &Connection,
  session_id: Option<&str>,
  sandbox_id: Option<&str>,
  cpu_percent: f64,
  memory_mb: f64,
  network_rx_bytes: f64,
  network_tx_bytes: f64,
) -> Result<ContainerMetric> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO container_metrics
       (id, session_id, sandbox_id, captured_at, cpu_percent, memory_mb, network_rx_bytes, network_tx_bytes)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    params![id, session_id, sandbox_id, now, cpu_percent, memory_mb, network_rx_bytes, network_tx_bytes],
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

pub fn list_for_sandbox(conn: &Connection, sandbox_id: &str) -> Result<Vec<ContainerMetric>> {
  let mut stmt =
    conn.prepare("SELECT * FROM container_metrics WHERE sandbox_id = ?1 ORDER BY captured_at ASC")?;
  let rows = stmt.query_map(params![sandbox_id], row_to_container_metric)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::{projects, sessions, tasks};

  fn make_session(conn: &Connection) -> String {
    let task = tasks::create(conn, "Task", None, None, "todo", None).unwrap();
    sessions::create(conn, &task.id, "claude_code", "autonomous", None).unwrap().id
  }

  fn make_sandbox(conn: &Connection) -> String {
    let project = projects::create(conn, "Overnight", "/repo/overnight", None, None, &[]).unwrap();
    crate::db::sandboxes::create(conn, &project.id, "mount", None, None, "default").unwrap().id
  }

  #[test]
  fn record_and_list_for_session() {
    let conn = test_conn();
    let session_id = make_session(&conn);

    record_for_session(&conn, &session_id, 12.5, 256.0, 100.0, 50.0).unwrap();
    record_for_session(&conn, &session_id, 40.0, 512.0, 200.0, 90.0).unwrap();

    let list = list_for_session(&conn, &session_id).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].cpu_percent, 40.0);
    assert_eq!(list[1].network_tx_bytes, 90.0);
    assert_eq!(list[1].sandbox_id, None);
  }

  #[test]
  fn record_and_list_for_sandbox() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);

    record_for_sandbox(&conn, &sandbox_id, 5.0, 128.0, 10.0, 5.0).unwrap();

    let list = list_for_sandbox(&conn, &sandbox_id).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].session_id, None);
    assert_eq!(list[0].sandbox_id.as_deref(), Some(sandbox_id.as_str()));
  }
}
