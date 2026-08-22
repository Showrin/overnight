use std::path::PathBuf;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tauri::{AppHandle, Manager};

use crate::db::error::Result;

pub mod activity;
// Not yet consumed — sandbox/container orchestration isn't built, so no
// caller records container resource usage yet.
#[allow(dead_code)]
pub mod container_metrics;
pub mod error;
pub mod jira_issues;
pub mod metrics;
pub mod migrations;
pub mod models;
pub mod projects;
// Not yet consumed — no Tauri commands wire into sandbox lifecycle yet
// (lands in a later Sandboxes-page step).
#[allow(dead_code)]
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
    migrations::migrations().to_latest(&mut conn)?;
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
