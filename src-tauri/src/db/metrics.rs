use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, Metric};

fn row_to_metric(row: &rusqlite::Row) -> rusqlite::Result<Metric> {
  Ok(Metric {
    id: row.get("id")?,
    session_id: row.get("session_id")?,
    captured_at: row.get("captured_at")?,
    tokens_input: row.get("tokens_input")?,
    tokens_output: row.get("tokens_output")?,
    cost_usd: row.get("cost_usd")?,
    model: row.get("model")?,
  })
}

pub fn record(
  conn: &Connection,
  session_id: &str,
  tokens_input: i64,
  tokens_output: i64,
  cost_usd: Option<f64>,
  model: Option<&str>,
) -> Result<Metric> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO metrics (id, session_id, captured_at, tokens_input, tokens_output, cost_usd, model)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![id, session_id, now, tokens_input, tokens_output, cost_usd, model],
  )?;
  let metric = conn.query_row("SELECT * FROM metrics WHERE id = ?1", params![id], row_to_metric)?;
  Ok(metric)
}

pub fn list_for_session(conn: &Connection, session_id: &str) -> Result<Vec<Metric>> {
  let mut stmt =
    conn.prepare("SELECT * FROM metrics WHERE session_id = ?1 ORDER BY captured_at ASC")?;
  let rows = stmt.query_map(params![session_id], row_to_metric)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn total_tokens_for_session(conn: &Connection, session_id: &str) -> Result<(i64, i64)> {
  conn
    .query_row(
      "SELECT COALESCE(SUM(tokens_input), 0), COALESCE(SUM(tokens_output), 0) FROM metrics WHERE session_id = ?1",
      params![session_id],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(Into::into)
}

pub fn total_tokens_for_sandbox(conn: &Connection, sandbox_id: &str) -> Result<(i64, i64)> {
  conn
    .query_row(
      "SELECT COALESCE(SUM(m.tokens_input), 0), COALESCE(SUM(m.tokens_output), 0)
       FROM metrics m JOIN sessions s ON s.id = m.session_id
       WHERE s.sandbox_id = ?1",
      params![sandbox_id],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(Into::into)
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
  fn record_and_sum() {
    let conn = test_conn();
    let session_id = make_session(&conn);

    record(&conn, &session_id, 100, 50, Some(0.01), Some("sonnet")).unwrap();
    record(&conn, &session_id, 200, 75, Some(0.02), Some("sonnet")).unwrap();

    let list = list_for_session(&conn, &session_id).unwrap();
    assert_eq!(list.len(), 2);

    let (input, output) = total_tokens_for_session(&conn, &session_id).unwrap();
    assert_eq!(input, 300);
    assert_eq!(output, 125);
  }

  #[test]
  fn sums_tokens_for_sandbox_across_sessions() {
    let conn = test_conn();
    let task = tasks::create(&conn, "Task", None, None, "todo", None).unwrap();
    let project = crate::db::projects::create(&conn, "Overnight", "/repo/overnight", None, None).unwrap();
    let sandbox = crate::db::sandboxes::create(&conn, &project.id, "mount", None, None, "default", None).unwrap();

    let session_a = sessions::create(&conn, &task.id, "claude_code", "autonomous", None).unwrap();
    sessions::set_sandbox_id(&conn, &session_a.id, &sandbox.id).unwrap();
    let session_b = sessions::create(&conn, &task.id, "claude_code", "autonomous", None).unwrap();
    sessions::set_sandbox_id(&conn, &session_b.id, &sandbox.id).unwrap();
    let unrelated_session = sessions::create(&conn, &task.id, "claude_code", "plan", None).unwrap();

    record(&conn, &session_a.id, 100, 50, None, None).unwrap();
    record(&conn, &session_b.id, 20, 10, None, None).unwrap();
    record(&conn, &unrelated_session.id, 999, 999, None, None).unwrap();

    let (input, output) = total_tokens_for_sandbox(&conn, &sandbox.id).unwrap();
    assert_eq!(input, 120);
    assert_eq!(output, 60);
  }
}
