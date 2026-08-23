use rusqlite_migration::{Migrations, M};

// Schema v1. Timestamps are INTEGER unix-epoch-milliseconds, all primary keys
// are TEXT UUIDv4. Foreign keys cascade on delete so removing a task/session
// cleans up its dependents without manual fan-out deletes in the repo layer.
//
// Schema v2 (OVN-55): added `projects` + `tasks.project_id`. The old
// free-text `tasks.project_path` column is left in place, superseded but
// not backfilled/dropped, to keep the migration non-destructive.
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
  ), M::up(
    "
    CREATE TABLE jira_issues (
      id TEXT PRIMARY KEY,
      key TEXT NOT NULL UNIQUE,
      summary TEXT NOT NULL,
      status TEXT NOT NULL,
      issue_type TEXT,
      priority TEXT,
      assignee TEXT,
      url TEXT NOT NULL,
      raw_fields TEXT NOT NULL,
      synced_at INTEGER NOT NULL,
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    );
    CREATE INDEX ix_jira_issues_status ON jira_issues(status);
    ",
  ), M::up(
    "
    CREATE TABLE projects (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      repo_path TEXT NOT NULL,
      plans_path TEXT,
      dev_server_port INTEGER,
      extra_clone_paths TEXT NOT NULL DEFAULT '[]',
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    );

    ALTER TABLE tasks ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;
    ",
  ), M::up(
    "
    CREATE TABLE sandboxes (
      id TEXT PRIMARY KEY,
      project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
      mode TEXT NOT NULL,
      status TEXT NOT NULL,
      container_id TEXT,
      folder_path TEXT,
      host_port INTEGER,
      created_at INTEGER NOT NULL,
      stopped_at INTEGER
    );
    CREATE INDEX ix_sandboxes_project_id ON sandboxes(project_id);

    -- container_metrics predates sandboxes and only ever recorded a required
    -- session_id. Recreated here (rather than ALTERed) so session_id can
    -- become optional and sandbox_id/network columns can be added in one
    -- consistent shape, preserving any existing rows.
    CREATE TABLE container_metrics_new (
      id TEXT PRIMARY KEY,
      session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
      sandbox_id TEXT REFERENCES sandboxes(id) ON DELETE CASCADE,
      captured_at INTEGER NOT NULL,
      cpu_percent REAL NOT NULL,
      memory_mb REAL NOT NULL,
      network_rx_bytes REAL NOT NULL DEFAULT 0,
      network_tx_bytes REAL NOT NULL DEFAULT 0
    );
    INSERT INTO container_metrics_new (id, session_id, captured_at, cpu_percent, memory_mb)
      SELECT id, session_id, captured_at, cpu_percent, memory_mb FROM container_metrics;
    DROP TABLE container_metrics;
    ALTER TABLE container_metrics_new RENAME TO container_metrics;
    CREATE INDEX ix_container_metrics_session_captured ON container_metrics(session_id, captured_at);
    CREATE INDEX ix_container_metrics_sandbox_captured ON container_metrics(sandbox_id, captured_at);
    ",
  ), M::up(
    "
    ALTER TABLE sessions ADD COLUMN sandbox_id TEXT REFERENCES sandboxes(id) ON DELETE SET NULL;
    CREATE INDEX ix_sessions_sandbox_id ON sessions(sandbox_id);
    ",
  ), M::up(
    "
    -- Sandboxes moved from raw docker containers to the sbx CLI (Docker
    -- Sandboxes), whose unit of identity is a sandbox name, not a
    -- container id.
    ALTER TABLE sandboxes RENAME COLUMN container_id TO sbx_name;
    ",
  ), M::up(
    "
    -- User-facing label, separate from sbx_name (the underlying sbx CLI
    -- identity) — lets the UI tell apart multiple clone-mode sandboxes
    -- for the same project.
    ALTER TABLE sandboxes ADD COLUMN name TEXT;
    ",
  ), M::up(
    "
    -- Snapshot of the Claude permission mode this sandbox was created
    -- with (one of plan/default/acceptEdits/bypassPermissions), applied
    -- both to app-launched autonomous sessions and to a bare `claude`
    -- typed in the sandbox's own terminal. Existing rows default to
    -- 'default' (Claude's own manual-approval mode) rather than the
    -- 'bypassPermissions' they were actually launched with previously,
    -- since that was undocumented app behavior, not a user choice.
    ALTER TABLE sandboxes ADD COLUMN permission_mode TEXT NOT NULL DEFAULT 'default';
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
    assert_eq!(table_count, 9);
  }
}
