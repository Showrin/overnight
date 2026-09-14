use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, CommandLogEntry};

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<CommandLogEntry> {
  Ok(CommandLogEntry {
    id: row.get("id")?,
    occurred_at: row.get("occurred_at")?,
    operation: row.get("operation")?,
    program: row.get("program")?,
    args: row.get("args")?,
    success: row.get("success")?,
    exit_code: row.get("exit_code")?,
    stderr: row.get("stderr")?,
  })
}

pub fn append(
  conn: &Connection,
  operation: &str,
  program: &str,
  args: &[String],
  success: bool,
  exit_code: Option<i64>,
  stderr: Option<&str>,
) -> Result<CommandLogEntry> {
  let id = new_id();
  let now = now_millis();
  let args_json = serde_json::to_string(args)?;
  conn.execute(
    "INSERT INTO command_log (id, occurred_at, operation, program, args, success, exit_code, stderr)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    params![id, now, operation, program, args_json, success, exit_code, stderr],
  )?;
  let entry = conn.query_row("SELECT * FROM command_log WHERE id = ?1", params![id], row_to_entry)?;
  Ok(entry)
}

/// Filters shared by `list` and `count` — kept as one struct so the two
/// queries can never drift out of sync on what "matching rows" means.
#[derive(Debug, Clone, Default)]
pub struct CommandLogFilter<'a> {
  pub success_only: Option<bool>,
  pub since_ms: Option<i64>,
  pub until_ms: Option<i64>,
  pub operation: Option<&'a str>,
}

fn where_clause(filter: &CommandLogFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
  let mut clauses: Vec<String> = Vec::new();
  let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

  if let Some(success) = filter.success_only {
    clauses.push(format!("success = ?{}", query_params.len() + 1));
    query_params.push(Box::new(success));
  }
  if let Some(since) = filter.since_ms {
    clauses.push(format!("occurred_at >= ?{}", query_params.len() + 1));
    query_params.push(Box::new(since));
  }
  if let Some(until) = filter.until_ms {
    clauses.push(format!("occurred_at <= ?{}", query_params.len() + 1));
    query_params.push(Box::new(until));
  }
  if let Some(operation) = filter.operation {
    clauses.push(format!("operation = ?{}", query_params.len() + 1));
    query_params.push(Box::new(operation.to_string()));
  }

  let sql = if clauses.is_empty() { String::new() } else { format!("WHERE {}", clauses.join(" AND ")) };
  (sql, query_params)
}

pub fn list(conn: &Connection, limit: i64, offset: i64, filter: &CommandLogFilter) -> Result<Vec<CommandLogEntry>> {
  let (where_sql, mut query_params) = where_clause(filter);
  let limit_idx = query_params.len() + 1;
  let offset_idx = query_params.len() + 2;
  let sql = format!("SELECT * FROM command_log {where_sql} ORDER BY occurred_at DESC LIMIT ?{limit_idx} OFFSET ?{offset_idx}");
  query_params.push(Box::new(limit));
  query_params.push(Box::new(offset));

  let mut stmt = conn.prepare(&sql)?;
  let param_refs: Vec<&dyn rusqlite::ToSql> = query_params.iter().map(|p| p.as_ref()).collect();
  let rows = stmt.query_map(param_refs.as_slice(), row_to_entry)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn count(conn: &Connection, filter: &CommandLogFilter) -> Result<i64> {
  let (where_sql, query_params) = where_clause(filter);
  let sql = format!("SELECT count(*) FROM command_log {where_sql}");
  let param_refs: Vec<&dyn rusqlite::ToSql> = query_params.iter().map(|p| p.as_ref()).collect();
  Ok(conn.query_row(&sql, param_refs.as_slice(), |row| row.get(0))?)
}

/// Every distinct operation label seen so far, for the Command Logs page's
/// action filter dropdown — populated from real data rather than a
/// hardcoded list, so it can't drift out of sync with the labels used
/// across `sbx.rs`/`commands.rs`.
pub fn list_operations(conn: &Connection) -> Result<Vec<String>> {
  let mut stmt = conn.prepare("SELECT DISTINCT operation FROM command_log ORDER BY operation")?;
  let rows = stmt.query_map([], |row| row.get(0))?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  #[test]
  fn append_and_list_most_recent_first() {
    let conn = test_conn();
    append(&conn, "Stop sandbox", "sbx", &["stop".to_string(), "foo".to_string()], true, Some(0), None).unwrap();
    append(&conn, "Remove sandbox", "sbx", &["rm".to_string(), "--force".to_string(), "foo".to_string()], false, Some(1), Some("boom")).unwrap();

    let entries = list(&conn, 10, 0, &CommandLogFilter::default()).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].operation, "Remove sandbox");
    assert_eq!(entries[0].stderr.as_deref(), Some("boom"));
    assert_eq!(entries[1].operation, "Stop sandbox");
  }

  #[test]
  fn filters_by_success() {
    let conn = test_conn();
    append(&conn, "Stop sandbox", "sbx", &["stop".to_string()], true, Some(0), None).unwrap();
    append(&conn, "Remove sandbox", "sbx", &["rm".to_string()], false, Some(1), Some("boom")).unwrap();

    let failed = list(&conn, 10, 0, &CommandLogFilter { success_only: Some(false), ..Default::default() }).unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].operation, "Remove sandbox");

    let succeeded = list(&conn, 10, 0, &CommandLogFilter { success_only: Some(true), ..Default::default() }).unwrap();
    assert_eq!(succeeded.len(), 1);
    assert_eq!(succeeded[0].operation, "Stop sandbox");
  }

  #[test]
  fn filters_by_operation() {
    let conn = test_conn();
    append(&conn, "Stop sandbox", "sbx", &["stop".to_string()], true, Some(0), None).unwrap();
    append(&conn, "Remove sandbox", "sbx", &["rm".to_string()], false, Some(1), Some("boom")).unwrap();

    let filtered = list(&conn, 10, 0, &CommandLogFilter { operation: Some("Stop sandbox"), ..Default::default() }).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].operation, "Stop sandbox");
  }

  #[test]
  fn respects_limit_and_offset() {
    let conn = test_conn();
    for i in 0..5 {
      append(&conn, &format!("op {i}"), "sbx", &[], true, Some(0), None).unwrap();
    }
    let page = list(&conn, 2, 1, &CommandLogFilter::default()).unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].operation, "op 3");
    assert_eq!(page[1].operation, "op 2");
  }

  #[test]
  fn filters_by_date_range() {
    let conn = test_conn();
    for (i, ts) in [1_000i64, 2_000, 3_000].into_iter().enumerate() {
      let id = new_id();
      conn
        .execute(
          "INSERT INTO command_log (id, occurred_at, operation, program, args, success, exit_code, stderr)
           VALUES (?1, ?2, ?3, 'sbx', '[]', 1, 0, NULL)",
          params![id, ts, format!("op {i}")],
        )
        .unwrap();
    }

    let within_range =
      list(&conn, 10, 0, &CommandLogFilter { since_ms: Some(1_500), until_ms: Some(2_500), ..Default::default() }).unwrap();
    assert_eq!(within_range.len(), 1);
    assert_eq!(within_range[0].operation, "op 1");

    let since_only = list(&conn, 10, 0, &CommandLogFilter { since_ms: Some(2_000), ..Default::default() }).unwrap();
    assert_eq!(since_only.len(), 2);
  }

  #[test]
  fn counts_matching_rows() {
    let conn = test_conn();
    append(&conn, "Stop sandbox", "sbx", &["stop".to_string()], true, Some(0), None).unwrap();
    append(&conn, "Remove sandbox", "sbx", &["rm".to_string()], false, Some(1), Some("boom")).unwrap();

    assert_eq!(count(&conn, &CommandLogFilter::default()).unwrap(), 2);
    assert_eq!(count(&conn, &CommandLogFilter { success_only: Some(false), ..Default::default() }).unwrap(), 1);
  }

  #[test]
  fn lists_distinct_operations_sorted() {
    let conn = test_conn();
    append(&conn, "Stop sandbox", "sbx", &[], true, Some(0), None).unwrap();
    append(&conn, "Stop sandbox", "sbx", &[], true, Some(0), None).unwrap();
    append(&conn, "Create sandbox", "sbx", &[], true, Some(0), None).unwrap();

    assert_eq!(list_operations(&conn).unwrap(), vec!["Create sandbox".to_string(), "Stop sandbox".to_string()]);
  }
}
