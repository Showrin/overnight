use std::path::PathBuf;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tauri::{AppHandle, Manager};

use crate::db::error::Result;

pub mod activity;
pub mod backups;
pub mod command_log;
pub mod container_metrics;
pub mod error;
pub mod host_metrics;
pub mod jira_issues;
pub mod metrics;
pub mod migrations;
pub mod models;
pub mod projects;
pub mod sandboxes;
pub mod sessions;
pub mod settings;
pub mod tasks;

pub type DbPool = Pool<SqliteConnectionManager>;

fn db_path(app: &AppHandle) -> Result<PathBuf> {
  let dir = app
    .path()
    .app_data_dir()
    .map_err(|e| error::Error::Sqlite(rusqlite::Error::InvalidPath(e.to_string().into())))?;
  std::fs::create_dir_all(&dir).ok();
  Ok(dir.join("overnight.sqlite"))
}

// Temporary diagnostic for tracking down a migration-counter/schema drift
// on an existing dev database — remove once the drift is understood and
// fixed. `rusqlite_migration` tracks progress as a plain `user_version`
// count, so it can drift out of sync with which tables actually exist if a
// database has been migrated by different versions of this migration list
// over time.
fn log_migration_state(conn: &rusqlite::Connection, when: &str) {
  let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap_or(-1);
  let tables: Vec<String> = conn
    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
    .and_then(|mut stmt| stmt.query_map([], |row| row.get(0))?.collect())
    .unwrap_or_default();
  log::info!("db: {when} migrations — user_version={version}, tables={tables:?}");

  if tables.iter().any(|t| t == "rusqlite_migration") {
    let rows: Vec<String> = conn
      .prepare("SELECT * FROM rusqlite_migration")
      .and_then(|mut stmt| {
        let col_count = stmt.column_count();
        stmt
          .query_map([], |row| {
            let cells: Vec<String> =
              (0..col_count).map(|i| row.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}")).unwrap_or_default()).collect();
            Ok(cells.join(", "))
          })?
          .collect()
      })
      .unwrap_or_default();
    log::info!("db: {when} migrations — rusqlite_migration rows: {rows:?}");
  }
}

pub fn init_pool(app: &AppHandle) -> Result<DbPool> {
  let path = db_path(app)?;

  // Run migrations through a single dedicated connection *before* the pool
  // exists. r2d2 opens up to max_size connections concurrently at build
  // time; racing several of them against DDL + the WAL pragma switch on a
  // fresh file causes transient "database is locked" errors. Doing this
  // once, sequentially, avoids that entirely.
  {
    let mut conn = rusqlite::Connection::open(&path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    log_migration_state(&conn, "before");
    migrations::migrations().to_latest(&mut conn)?;
    log_migration_state(&conn, "after");
  }

  let manager = SqliteConnectionManager::file(path).with_init(|conn| {
    conn.execute_batch(
      "PRAGMA foreign_keys = ON;
       PRAGMA journal_mode = WAL;
       PRAGMA busy_timeout = 5000;",
    )
  });
  let pool = Pool::builder().max_size(5).min_idle(Some(1)).build(manager)?;
  Ok(pool)
}
