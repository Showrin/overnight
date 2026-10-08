use rusqlite::{params, Connection, OptionalExtension};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, BrowserTest};

fn row_to_browser_test(row: &rusqlite::Row) -> rusqlite::Result<BrowserTest> {
  Ok(BrowserTest {
    id: row.get("id")?,
    sandbox_id: row.get("sandbox_id")?,
    status: row.get("status")?,
    doc: row.get("doc")?,
    branch: row.get("branch")?,
    sandbox_port: row.get("sandbox_port")?,
    target_url: row.get("target_url")?,
    verdict: row.get("verdict")?,
    report: row.get("report")?,
    error: row.get("error")?,
    created_at: row.get("created_at")?,
    started_at: row.get("started_at")?,
    finished_at: row.get("finished_at")?,
  })
}

/// `status` is "queued" or "awaiting_host" — see `BrowserTest::status`.
pub fn create(
  conn: &Connection,
  sandbox_id: &str,
  doc: &str,
  branch: Option<&str>,
  sandbox_port: Option<i64>,
  status: &str,
) -> Result<BrowserTest> {
  if doc.trim().is_empty() {
    return Err(Error::InvalidValue("test doc is empty".to_string()));
  }
  let id = new_id();
  conn.execute(
    "INSERT INTO browser_tests (id, sandbox_id, status, doc, branch, sandbox_port, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![id, sandbox_id, status, doc, branch, sandbox_port, now_millis()],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<BrowserTest> {
  conn
    .query_row("SELECT * FROM browser_tests WHERE id = ?1", params![id], row_to_browser_test)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list_for_sandbox(conn: &Connection, sandbox_id: &str) -> Result<Vec<BrowserTest>> {
  let mut stmt = conn.prepare("SELECT * FROM browser_tests WHERE sandbox_id = ?1 ORDER BY created_at DESC")?;
  let rows = stmt.query_map(params![sandbox_id], row_to_browser_test)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Atomically takes the oldest queued test and marks it running, so only
/// one runner can ever pick a given test up.
pub fn claim_next_queued(conn: &Connection) -> Result<Option<BrowserTest>> {
  Ok(
    conn
      .query_row(
        "UPDATE browser_tests SET status = 'running', started_at = ?1
         WHERE id = (SELECT id FROM browser_tests WHERE status = 'queued' ORDER BY created_at LIMIT 1)
         RETURNING *",
        params![now_millis()],
        row_to_browser_test,
      )
      .optional()?,
  )
}

/// Moves an "awaiting_host" test into the queue once the user says the
/// host-side servers are up.
pub fn release_to_queue(conn: &Connection, id: &str) -> Result<BrowserTest> {
  transition(conn, id, "UPDATE browser_tests SET status = 'queued' WHERE id = ?1 AND status = 'awaiting_host'")
}

/// Cancels a test that hasn't started yet. A running test is stopped by
/// the runner itself (see `crate::browser_tests::cancel`).
pub fn cancel_pending(conn: &Connection, id: &str) -> Result<BrowserTest> {
  conn.execute(
    "UPDATE browser_tests SET status = 'cancelled', finished_at = ?1 WHERE id = ?2 AND status IN ('awaiting_host', 'queued')",
    params![now_millis(), id],
  )?;
  get(conn, id)
}

pub fn set_target_url(conn: &Connection, id: &str, url: &str) -> Result<()> {
  conn.execute("UPDATE browser_tests SET target_url = ?1 WHERE id = ?2", params![url, id])?;
  Ok(())
}

/// Records how a running test ended. Ignored unless the test is still
/// "running" — a cancel that raced the finish keeps its "cancelled".
pub fn finish(
  conn: &Connection,
  id: &str,
  status: &str,
  verdict: Option<&str>,
  report: Option<&str>,
  error: Option<&str>,
) -> Result<BrowserTest> {
  conn.execute(
    "UPDATE browser_tests SET status = ?1, verdict = ?2, report = ?3, error = ?4, finished_at = ?5
     WHERE id = ?6 AND status = 'running'",
    params![status, verdict, report, error, now_millis(), id],
  )?;
  get(conn, id)
}

pub fn mark_cancelled(conn: &Connection, id: &str) -> Result<()> {
  conn.execute(
    "UPDATE browser_tests SET status = 'cancelled', finished_at = ?1 WHERE id = ?2 AND status = 'running'",
    params![now_millis(), id],
  )?;
  Ok(())
}

/// A test left "running" by a previous app session can't still be running
/// — its host agent died with the app. Called once at startup.
pub fn fail_interrupted(conn: &Connection) -> Result<usize> {
  Ok(conn.execute(
    "UPDATE browser_tests SET status = 'failed', error = 'Overnight quit while this test was running', finished_at = ?1
     WHERE status = 'running'",
    params![now_millis()],
  )?)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  conn.execute("DELETE FROM browser_tests WHERE id = ?1 AND status NOT IN ('running')", params![id])?;
  Ok(())
}

fn transition(conn: &Connection, id: &str, sql: &str) -> Result<BrowserTest> {
  let changed = conn.execute(sql, params![id])?;
  let test = get(conn, id)?;
  if changed == 0 {
    return Err(Error::InvalidValue(format!("browser test is {}", test.status)));
  }
  Ok(test)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  fn make_sandbox(conn: &Connection) -> String {
    let project_id = crate::db::projects::create(conn, "Overnight", "/repo/overnight", None, None).unwrap().id;
    crate::db::sandboxes::create(conn, &project_id, "mount", None, None, "default", None, "claude").unwrap().id
  }

  #[test]
  fn queued_tests_are_claimed_oldest_first_and_only_once() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let first = create(&conn, &sandbox_id, "check login", Some("feat/login"), None, "queued").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let second = create(&conn, &sandbox_id, "check signup", None, Some(5173), "queued").unwrap();

    let claimed = claim_next_queued(&conn).unwrap().unwrap();
    assert_eq!(claimed.id, first.id);
    assert_eq!(claimed.status, "running");
    assert!(claimed.started_at.is_some());
    assert_eq!(claim_next_queued(&conn).unwrap().unwrap().id, second.id);
    assert!(claim_next_queued(&conn).unwrap().is_none());
  }

  #[test]
  fn awaiting_host_tests_wait_until_released() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let test = create(&conn, &sandbox_id, "check login", None, None, "awaiting_host").unwrap();
    assert!(claim_next_queued(&conn).unwrap().is_none());

    assert_eq!(release_to_queue(&conn, &test.id).unwrap().status, "queued");
    assert!(release_to_queue(&conn, &test.id).is_err());
    assert_eq!(claim_next_queued(&conn).unwrap().unwrap().id, test.id);
  }

  #[test]
  fn finish_records_report_but_not_over_a_cancel() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let a = create(&conn, &sandbox_id, "a", None, None, "queued").unwrap();
    claim_next_queued(&conn).unwrap();
    let done = finish(&conn, &a.id, "done", Some("pass"), Some("# ok"), None).unwrap();
    assert_eq!(done.status, "done");
    assert_eq!(done.verdict.as_deref(), Some("pass"));
    assert!(done.finished_at.is_some());

    let b = create(&conn, &sandbox_id, "b", None, None, "queued").unwrap();
    claim_next_queued(&conn).unwrap();
    mark_cancelled(&conn, &b.id).unwrap();
    assert_eq!(finish(&conn, &b.id, "failed", None, None, Some("killed")).unwrap().status, "cancelled");
  }

  #[test]
  fn cancel_pending_and_fail_interrupted() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let pending = create(&conn, &sandbox_id, "a", None, None, "awaiting_host").unwrap();
    assert_eq!(cancel_pending(&conn, &pending.id).unwrap().status, "cancelled");

    let running = create(&conn, &sandbox_id, "b", None, None, "queued").unwrap();
    claim_next_queued(&conn).unwrap();
    assert_eq!(fail_interrupted(&conn).unwrap(), 1);
    let failed = get(&conn, &running.id).unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.error.is_some());
  }

  #[test]
  fn rejects_empty_doc() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    assert!(create(&conn, &sandbox_id, "  ", None, None, "queued").is_err());
  }
}
