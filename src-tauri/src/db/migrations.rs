use rusqlite_migration::{Migrations, M};

// Schema v1. Timestamps are INTEGER unix-epoch-milliseconds, all primary keys
// are TEXT UUIDv4. Foreign keys cascade on delete so removing a task/session
// cleans up its dependents without manual fan-out deletes in the repo layer.
//
// Forward-compat note: a future schema v2 will likely add a proper
// `project_folders` table + `project_folder_id` FK once OVN-55 lands,
// deprecating the free-text `tasks.project_path` column used for now.
pub fn migrations() -> Migrations<'static> {
  Migrations::new(vec![M::up(
    "
    CREATE TABLE tasks (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      jira_key TEXT,
      status TEXT NOT NULL,
      project_path TEXT,
      metadata TEXT,
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    );
    CREATE UNIQUE INDEX ux_tasks_jira_key ON tasks(jira_key) WHERE jira_key IS NOT NULL;

    CREATE TABLE sessions (
      id TEXT PRIMARY KEY,
      task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
      agent_provider TEXT NOT NULL,
      provider_session_id TEXT,
      parent_session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
      mode TEXT NOT NULL,
      status TEXT NOT NULL,
      started_at INTEGER NOT NULL,
      ended_at INTEGER,
      transcript_path TEXT,
      plan_path TEXT
    );
    CREATE INDEX ix_sessions_task_id ON sessions(task_id);

    CREATE TABLE activity (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      occurred_at INTEGER NOT NULL,
      event_type TEXT NOT NULL,
      payload TEXT NOT NULL
    );
    CREATE INDEX ix_activity_session_occurred ON activity(session_id, occurred_at);

    CREATE TABLE metrics (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      captured_at INTEGER NOT NULL,
      tokens_input INTEGER NOT NULL,
      tokens_output INTEGER NOT NULL,
      cost_usd REAL,
      model TEXT
    );
    CREATE INDEX ix_metrics_session_captured ON metrics(session_id, captured_at);

    CREATE TABLE container_metrics (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      captured_at INTEGER NOT NULL,
      cpu_percent REAL NOT NULL,
      memory_mb REAL NOT NULL
    );
    CREATE INDEX ix_container_metrics_session_captured ON container_metrics(session_id, captured_at);

    CREATE TABLE settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at INTEGER NOT NULL
    );
    ",
  )])
}

#[cfg(test)]
pub fn test_conn() -> rusqlite::Connection {
  let mut conn = rusqlite::Connection::open_in_memory().unwrap();
  conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
  migrations().to_latest(&mut conn).unwrap();
  conn
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn migrations_apply_cleanly() {
    let conn = test_conn();
    let table_count: i64 = conn
      .query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != 'rusqlite_migration'",
        [],
        |row| row.get(0),
      )
      .unwrap();
    assert_eq!(table_count, 6);
  }
}
