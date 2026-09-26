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

fn dir_has_entries(path: &std::path::Path) -> bool {
  std::fs::read_dir(path).map(|mut entries| entries.next().is_some()).unwrap_or(false)
}

/// Cheap on-disk check (no deep walk) that every half the row claims is still there.
fn integrity_warnings(host_dir: &str, has_claude: bool, has_codex: bool, has_git: bool) -> Vec<String> {
  let root = std::path::Path::new(host_dir);
  if !root.is_dir() {
    return vec!["Backup folder is missing".to_string()];
  }
  [(has_claude, "claude", ".claude"), (has_codex, "codex", ".codex"), (has_git, "git", ".git")]
    .into_iter()
    .filter(|(present, sub, _)| *present && !dir_has_entries(&root.join(sub)))
    .map(|(_, _, label)| format!("{label} copy is missing or empty"))
    .collect()
}

fn row_to_backup(row: &rusqlite::Row) -> rusqlite::Result<SandboxBackup> {
  let branches_raw: String = row.get("branches")?;
  let host_dir: String = row.get("host_dir")?;
  let size_bytes: Option<i64> = row.get("size_bytes")?;
  let has_claude: bool = row.get("has_claude")?;
  let has_codex: bool = row.get("has_codex")?;
  let has_git: bool = row.get("has_git")?;
  let warnings_raw: String = row.get("warnings")?;
  let mut warnings: Vec<String> = serde_json::from_str(&warnings_raw).unwrap_or_default();
  warnings.extend(integrity_warnings(&host_dir, has_claude, has_codex, has_git));
  Ok(SandboxBackup {
    id: row.get("id")?,
    sandbox_id: row.get("sandbox_id")?,
    sandbox_label: row.get("sandbox_label")?,
    created_at: row.get("created_at")?,
    trigger: row.get("trigger")?,
    host_dir,
    has_claude,
    has_codex,
    has_git,
    base_branch: row.get("base_branch")?,
    current_branch: row.get("current_branch")?,
    branches: serde_json::from_str(&branches_raw).unwrap_or_default(),
    plan_file_count: row.get("plan_file_count")?,
    size_bytes: size_bytes.unwrap_or(0),
    warnings,
  })
}

/// Rows made before `size_bytes` existed get their size computed once, here.
fn backfill_missing_sizes(conn: &Connection) -> Result<()> {
  let mut stmt = conn.prepare("SELECT id, host_dir FROM sandbox_backups WHERE size_bytes IS NULL")?;
  let missing = stmt
    .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
    .collect::<rusqlite::Result<Vec<_>>>()?;
  for (id, host_dir) in missing {
    conn.execute("UPDATE sandbox_backups SET size_bytes = ?1 WHERE id = ?2", params![dir_size_bytes(&host_dir), id])?;
  }
  Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn insert(
  conn: &Connection,
  sandbox_id: &str,
  sandbox_label: Option<&str>,
  trigger: &str,
  host_dir: &str,
  has_claude: bool,
  has_codex: bool,
  has_git: bool,
  base_branch: Option<&str>,
  current_branch: Option<&str>,
  branches: &[String],
  plan_file_count: i64,
  warnings: &[String],
) -> Result<SandboxBackup> {
  let id = new_id();
  let now = now_millis();
  let branches_json = serde_json::to_string(branches).unwrap_or_else(|_| "[]".to_string());
  let warnings_json = serde_json::to_string(warnings).unwrap_or_else(|_| "[]".to_string());
  conn.execute(
    "INSERT INTO sandbox_backups (
       id, sandbox_id, sandbox_label, created_at, trigger, host_dir, has_claude, has_codex, has_git,
       base_branch, current_branch, branches, plan_file_count, size_bytes, warnings
     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    params![
      id,
      sandbox_id,
      sandbox_label,
      now,
      trigger,
      host_dir,
      has_claude,
      has_codex,
      has_git,
      base_branch,
      current_branch,
      branches_json,
      plan_file_count,
      dir_size_bytes(host_dir),
      warnings_json
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
  backfill_missing_sizes(conn)?;
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

pub fn sandbox_ids(conn: &Connection) -> Result<Vec<String>> {
  let mut stmt = conn.prepare("SELECT DISTINCT sandbox_id FROM sandbox_backups")?;
  let rows = stmt.query_map([], |r| r.get(0))?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_for_sandbox(conn: &Connection, sandbox_id: &str) -> Result<Vec<SandboxBackup>> {
  backfill_missing_sizes(conn)?;
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
    crate::db::sandboxes::create(conn, &project.id, "mount", None, None, "default", None, "claude").unwrap().id
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
      false,
      true,
      Some("main"),
      Some("feature"),
      &branches,
      3,
      &[],
    )
    .unwrap();

    assert_eq!(backup.sandbox_id, sandbox_id);
    assert_eq!(backup.sandbox_label.as_deref(), Some("my-sbx"));
    assert_eq!(backup.branches, branches);
    assert!(backup.has_claude);
    assert!(!backup.has_codex);
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
  fn insert_stores_size_and_list_backfills_null_sizes() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let dir = std::env::temp_dir().join(format!("overnight-size-test-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "12345").unwrap();

    let backup =
      insert(&conn, &sandbox_id, None, "manual", &dir.to_string_lossy(), true, false, false, None, None, &[], 0, &[]).unwrap();
    assert_eq!(backup.size_bytes, 5);

    conn.execute("UPDATE sandbox_backups SET size_bytes = NULL WHERE id = ?1", params![backup.id]).unwrap();
    let listed = list_all(&conn).unwrap();
    assert_eq!(listed[0].size_bytes, 5);
    let stored: Option<i64> =
      conn.query_row("SELECT size_bytes FROM sandbox_backups WHERE id = ?1", params![backup.id], |r| r.get(0)).unwrap();
    assert_eq!(stored, Some(5));

    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn stored_warnings_are_returned() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let dir = std::env::temp_dir().join(format!("overnight-warn-test-{}", new_id()));
    std::fs::create_dir_all(dir.join("claude")).unwrap();
    std::fs::write(dir.join("claude").join("a"), "x").unwrap();

    let warnings = vec![".git copy failed".to_string()];
    let backup =
      insert(&conn, &sandbox_id, None, "manual", &dir.to_string_lossy(), true, false, false, None, None, &[], 0, &warnings)
        .unwrap();
    assert_eq!(backup.warnings, warnings);

    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn integrity_warnings_flag_missing_parts() {
    let dir = std::env::temp_dir().join(format!("overnight-integrity-test-{}", new_id()));
    let host_dir = dir.to_string_lossy().to_string();
    assert_eq!(integrity_warnings(&host_dir, true, false, true), vec!["Backup folder is missing".to_string()]);

    std::fs::create_dir_all(dir.join("claude")).unwrap();
    std::fs::write(dir.join("claude").join("a"), "x").unwrap();
    std::fs::create_dir_all(dir.join("git")).unwrap();
    assert_eq!(integrity_warnings(&host_dir, true, false, true), vec![".git copy is missing or empty".to_string()]);

    std::fs::remove_dir_all(&dir).ok();
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
        insert(&conn, &sandbox_id, None, "scheduled", &format!("/data/{i}"), true, false, true, None, None, &[], 0, &[]).unwrap();
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
    insert(&conn, &sandbox_id, None, "scheduled", "/data/1", true, false, true, None, None, &[], 0, &[]).unwrap();

    assert!(beyond_limit(&conn, &sandbox_id, 10).unwrap().is_empty());
  }

  // Backups intentionally outlive their sandbox now — the on-disk copy is
  // still there and still restorable into some other sandbox, so deleting
  // the sandbox row must not take the backup history down with it.
  #[test]
  fn deleting_sandbox_does_not_delete_backups() {
    let conn = test_conn();
    let sandbox_id = make_sandbox(&conn);
    let backup = insert(&conn, &sandbox_id, Some("my-sbx"), "scheduled", "/data/1", true, false, true, None, None, &[], 0, &[]).unwrap();

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
    insert(&conn, &sandbox_id, None, "scheduled", "/data/1", true, false, true, None, None, &[], 0, &[]).unwrap();
    insert(&conn, &sandbox_id, None, "scheduled", "/data/2", true, false, true, None, None, &[], 0, &[]).unwrap();
    insert(&conn, &other_id, None, "scheduled", "/data/3", true, false, true, None, None, &[], 0, &[]).unwrap();

    assert_eq!(list_for_sandbox(&conn, &sandbox_id).unwrap().len(), 2);

    delete_all_for_sandbox(&conn, &sandbox_id).unwrap();

    assert!(list_for_sandbox(&conn, &sandbox_id).unwrap().is_empty());
    assert_eq!(list_all(&conn).unwrap().len(), 1);
  }
}
