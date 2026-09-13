use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, SandboxBackup};

/// Recursively sums file sizes under `path`. Best-effort: any unreadable
/// entry (permissions, a broken symlink) is just skipped rather than
/// failing the whole backup listing over one bad directory.
fn dir_size_bytes(path: &str) -> i64 {
  fn walk(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
      .flatten()
      .map(|entry| match entry.metadata() {
        Ok(metadata) if metadata.is_dir() => walk(&entry.path()),
        Ok(metadata) => metadata.len(),
        Err(_) => 0,
      })
      .sum()
  }
  walk(std::path::Path::new(path)) as i64
}

fn row_to_backup(row: &rusqlite::Row) -> rusqlite::Result<SandboxBackup> {
  let branches_raw: String = row.get("branches")?;
  let host_dir: String = row.get("host_dir")?;
  let size_bytes = dir_size_bytes(&host_dir);
  Ok(SandboxBackup {
    id: row.get("id")?,
    sandbox_id: row.get("sandbox_id")?,
    sandbox_label: row.get("sandbox_label")?,
    created_at: row.get("created_at")?,
    trigger: row.get("trigger")?,
    host_dir,
    has_claude: row.get("has_claude")?,
    has_git: row.get("has_git")?,
    base_branch: row.get("base_branch")?,
    current_branch: row.get("current_branch")?,
    branches: serde_json::from_str(&branches_raw).unwrap_or_default(),
    plan_file_count: row.get("plan_file_count")?,
    size_bytes,
  })
}

#[allow(clippy::too_many_arguments)]
pub fn insert(
  conn: &Connection,
  sandbox_id: &str,
  sandbox_label: Option<&str>,
  trigger: &str,
  host_dir: &str,
  has_claude: bool,
  has_git: bool,
  base_branch: Option<&str>,
  current_branch: Option<&str>,
  branches: &[String],
  plan_file_count: i64,
) -> Result<SandboxBackup> {
  let id = new_id();
  let now = now_millis();
  let branches_json = serde_json::to_string(branches).unwrap_or_else(|_| "[]".to_string());
  conn.execute(
    "INSERT INTO sandbox_backups (
       id, sandbox_id, sandbox_label, created_at, trigger, host_dir, has_claude, has_git,
       base_branch, current_branch, branches, plan_file_count
     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    params![
      id,
      sandbox_id,
      sandbox_label,
      now,
      trigger,
      host_dir,
      has_claude,
      has_git,
      base_branch,
      current_branch,
      branches_json,
      plan_file_count
    ],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<SandboxBackup> {
  conn
    .query_row("SELECT * FROM sandbox_backups WHERE id = ?1", params![id], row_to_backup)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list_all(conn: &Connection) -> Result<Vec<SandboxBackup>> {
  let mut stmt = conn.prepare("SELECT * FROM sandbox_backups ORDER BY created_at DESC")?;
  let rows = stmt.query_map([], row_to_backup)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Rows for `sandbox_id` beyond the newest `keep` — the ones pruning should
/// remove (both the row and its `host_dir` on disk).
pub fn beyond_limit(conn: &Connection, sandbox_id: &str, keep: i64) -> Result<Vec<SandboxBackup>> {
  let mut stmt =
    conn.prepare("SELECT * FROM sandbox_backups WHERE sandbox_id = ?1 ORDER BY created_at DESC LIMIT -1 OFFSET ?2")?;
  let rows = stmt.query_map(params![sandbox_id, keep], row_to_backup)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_for_sandbox(conn: &Connection, sandbox_id: &str) -> Result<Vec<SandboxBackup>> {
  let mut stmt = conn.prepare("SELECT * FROM sandbox_backups WHERE sandbox_id = ?1 ORDER BY created_at DESC")?;
  let rows = stmt.query_map(params![sandbox_id], row_to_backup)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  let changed = conn.execute("DELETE FROM sandbox_backups WHERE id = ?1", params![id])?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  Ok(())
}

pub fn delete_all_for_sandbox(conn: &Connection, sandbox_id: &str) -> Result<()> {
  conn.execute("DELETE FROM sandbox_backups WHERE sandbox_id = ?1", params![sandbox_id])?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  fn make_sandbox(conn: &Connection) -> String {
    let project = crate::db::projects::create(conn, "Overnight", "/repo/overnight", None, None).unwrap();
    crate::db::sandboxes::create(conn, &project.id, "mount", None, None, "default", None).unwrap().id
  }

  #[test]
  fn insert_get_list() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);

    let branches = vec!["main".to_string(), "feature".to_string()];
    let backup = insert(
      &conn,
      &sandbox_id,
      Some("my-sbx"),
      "scheduled",
      "/data/sandbox-backups/my-sbx/abc",
      true,
      true,
      Some("main"),
      Some("feature"),
      &branches,
      3,
    )
    .unwrap();

    assert_eq!(backup.sandbox_id, sandbox_id);
    assert_eq!(backup.sandbox_label.as_deref(), Some("my-sbx"));
    assert_eq!(backup.branches, branches);
    assert!(backup.has_claude);
    assert!(backup.has_git);
    assert_eq!(backup.plan_file_count, 3);
    // Nonexistent host_dir: sums to 0 rather than erroring.
    assert_eq!(backup.size_bytes, 0);

    let fetched = get(&conn, &backup.id).unwrap();
    assert_eq!(fetched.id, backup.id);

    let all = list_all(&conn).unwrap();
    assert_eq!(all.len(), 1);
  }

  #[test]
  fn get_missing_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(get(&conn, "missing"), Err(Error::NotFound)));
  }

  #[test]
  fn delete_missing_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(delete(&conn, "missing"), Err(Error::NotFound)));
  }

  #[test]
  fn beyond_limit_returns_oldest_rows_past_keep() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);

    let mut ids = Vec::new();
    for i in 0..12 {
      let backup =
        insert(&conn, &sandbox_id, None, "scheduled", &format!("/data/{i}"), true, true, None, None, &[], 0).unwrap();
      // created_at is millis-now for every insert in this test; force a
      // strictly increasing order so LIMIT/OFFSET has a stable sort key.
      conn
        .execute("UPDATE sandbox_backups SET created_at = ?1 WHERE id = ?2", params![i, backup.id])
        .unwrap();
      ids.push(backup.id);
    }

    let victims = beyond_limit(&conn, &sandbox_id, 10).unwrap();
    assert_eq!(victims.len(), 2);
    // Oldest two (created_at 0 and 1) are the ones beyond the newest 10.
    assert_eq!(victims.iter().map(|b| b.id.clone()).collect::<Vec<_>>(), vec![ids[1].clone(), ids[0].clone()]);
  }

  #[test]
  fn beyond_limit_empty_when_under_keep() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    insert(&conn, &sandbox_id, None, "scheduled", "/data/1", true, true, None, None, &[], 0).unwrap();

    assert!(beyond_limit(&conn, &sandbox_id, 10).unwrap().is_empty());
  }

  // Backups intentionally outlive their sandbox now — the on-disk copy is
  // still there and still restorable into some other sandbox, so deleting
  // the sandbox row must not take the backup history down with it.
  #[test]
  fn deleting_sandbox_does_not_delete_backups() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let backup = insert(&conn, &sandbox_id, Some("my-sbx"), "scheduled", "/data/1", true, true, None, None, &[], 0).unwrap();

    crate::db::sandboxes::delete(&conn, &sandbox_id).unwrap();

    let all = list_all(&conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, backup.id);
    assert_eq!(all[0].sandbox_label.as_deref(), Some("my-sbx"));
  }

  #[test]
  fn list_for_sandbox_filters_and_delete_all_for_sandbox_clears_them() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let other_id = make_sandbox(&conn);
    insert(&conn, &sandbox_id, None, "scheduled", "/data/1", true, true, None, None, &[], 0).unwrap();
    insert(&conn, &sandbox_id, None, "scheduled", "/data/2", true, true, None, None, &[], 0).unwrap();
    insert(&conn, &other_id, None, "scheduled", "/data/3", true, true, None, None, &[], 0).unwrap();

    assert_eq!(list_for_sandbox(&conn, &sandbox_id).unwrap().len(), 2);

    delete_all_for_sandbox(&conn, &sandbox_id).unwrap();

    assert!(list_for_sandbox(&conn, &sandbox_id).unwrap().is_empty());
    assert_eq!(list_all(&conn).unwrap().len(), 1);
  }
}
