use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, EnvVar, Project, Secret};

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
  let extra_clone_paths_raw: String = row.get("extra_clone_paths")?;
  let env_vars_raw: String = row.get("env_vars")?;
  let secrets_raw: String = row.get("secrets")?;
  Ok(Project {
    id: row.get("id")?,
    name: row.get("name")?,
    repo_path: row.get("repo_path")?,
    extra_clone_paths: serde_json::from_str(&extra_clone_paths_raw).unwrap_or_default(),
    env_vars: serde_json::from_str(&env_vars_raw).unwrap_or_default(),
    secrets: serde_json::from_str(&secrets_raw).unwrap_or_default(),
    kit_id: row.get("kit_id")?,
    compose_override: row.get("compose_override")?,
    created_at: row.get("created_at")?,
    updated_at: row.get("updated_at")?,
  })
}

pub fn create(conn: &Connection, name: &str, repo_path: &str) -> Result<Project> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO projects (id, name, repo_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
    params![id, name, repo_path, now],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Project> {
  conn
    .query_row("SELECT * FROM projects WHERE id = ?1", params![id], row_to_project)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Project>> {
  let mut stmt = conn.prepare("SELECT * FROM projects ORDER BY updated_at DESC")?;
  let rows = stmt.query_map([], row_to_project)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn update(conn: &Connection, id: &str, name: &str, repo_path: &str) -> Result<Project> {
  let now = now_millis();
  let changed = conn.execute(
    "UPDATE projects SET name = ?1, repo_path = ?2, updated_at = ?3 WHERE id = ?4",
    params![name, repo_path, now, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

/// Persists this project's env vars, applied to every sandbox created for
/// it from then on (merged with global and sandbox-scoped vars at sandbox
/// creation time — see commands.rs::merged_env_vars). Bumps updated_at,
/// same as `update`.
pub fn set_env_vars(conn: &Connection, id: &str, vars: &[EnvVar]) -> Result<Project> {
  let json = serde_json::to_string(vars).unwrap_or_else(|_| "[]".to_string());
  let now = now_millis();
  let changed = conn.execute(
    "UPDATE projects SET env_vars = ?1, updated_at = ?2 WHERE id = ?3",
    params![json, now, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

/// Persists this project's secrets, applied to every sandbox created for
/// it from then on (merged with global and sandbox-scoped secrets at
/// sandbox creation time — see commands.rs::merged_secrets). Bumps
/// updated_at, same as `set_env_vars`.
pub fn set_secrets(conn: &Connection, id: &str, secrets: &[Secret]) -> Result<Project> {
  let json = serde_json::to_string(secrets).unwrap_or_else(|_| "[]".to_string());
  let now = now_millis();
  let changed = conn.execute(
    "UPDATE projects SET secrets = ?1, updated_at = ?2 WHERE id = ?3",
    params![json, now, id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn set_compose_override(conn: &Connection, id: &str, content: Option<&str>) -> Result<Project> {
  let changed = conn.execute(
    "UPDATE projects SET compose_override = ?1, updated_at = ?2 WHERE id = ?3",
    params![content, now_millis(), id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  let changed = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  Ok(())
}

/// Fixed id for the singleton "Unassigned" project — never a
/// dangling/sentinel id since `sandboxes.project_id` is `NOT NULL
/// REFERENCES projects(id) ON DELETE CASCADE` (foreign keys are enforced),
/// so orphan-adopted sandboxes with no matching project need a real row.
pub const UNASSIGNED_PROJECT_ID: &str = "unassigned";

/// Returns the singleton "Unassigned" project, creating it the first time
/// it's needed (e.g. the first orphan-adopted sandbox with no matching
/// `repo_path`) rather than requiring it to exist up front. `repo_path` is
/// deliberately empty — this project never backs a real host checkout, so
/// nothing should read it expecting a usable path (adopted sandboxes
/// assigned here are always `mode = "clone"` with no `folder_path`).
pub fn get_or_create_unassigned(conn: &Connection) -> Result<Project> {
  if let Ok(project) = get(conn, UNASSIGNED_PROJECT_ID) {
    return Ok(project);
  }
  let now = now_millis();
  let result = conn.execute(
    "INSERT INTO projects (id, name, repo_path, created_at, updated_at) VALUES (?1, 'Unassigned', '', ?2, ?2)",
    params![UNASSIGNED_PROJECT_ID, now],
  );
  // Tolerate a concurrent creator winning the race (primary key conflict) —
  // any other failure still propagates.
  if let Err(e) = result {
    if e.sqlite_extended_error_code() != Some(rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY) {
      return Err(Error::Sqlite(e));
    }
  }
  get(conn, UNASSIGNED_PROJECT_ID)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::models::{SecretSource, SecretTarget};

  #[test]
  fn set_compose_override_round_trips() {
    let conn = test_conn();
    let project = create(&conn, "P", "/repo/p").unwrap();
    assert_eq!(project.compose_override, None);
    let updated = set_compose_override(&conn, &project.id, Some("services: {}")).unwrap();
    assert_eq!(updated.compose_override.as_deref(), Some("services: {}"));
    assert_eq!(set_compose_override(&conn, &project.id, None).unwrap().compose_override, None);
  }

  #[test]
  fn create_get_list_update_delete() {
    let conn = test_conn();

    let project = create(&conn, "Overnight", "/repo/overnight").unwrap();
    assert_eq!(project.name, "Overnight");

    let fetched = get(&conn, &project.id).unwrap();
    assert_eq!(fetched.id, project.id);

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);

    let updated = update(&conn, &project.id, "Overnight Renamed", "/repo/overnight").unwrap();
    assert_eq!(updated.name, "Overnight Renamed");
    assert!(updated.updated_at >= project.updated_at);

    delete(&conn, &project.id).unwrap();
    assert!(matches!(get(&conn, &project.id), Err(Error::NotFound)));
  }

  #[test]
  fn missing_id_operations_return_not_found() {
    let conn = test_conn();
    assert!(matches!(get(&conn, "missing"), Err(Error::NotFound)));
    assert!(matches!(update(&conn, "missing", "x", "/repo"), Err(Error::NotFound)));
    assert!(matches!(delete(&conn, "missing"), Err(Error::NotFound)));
  }

  #[test]
  fn get_or_create_unassigned_is_a_singleton() {
    let conn = test_conn();

    let first = get_or_create_unassigned(&conn).unwrap();
    assert_eq!(first.id, UNASSIGNED_PROJECT_ID);
    assert_eq!(first.name, "Unassigned");

    let second = get_or_create_unassigned(&conn).unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(second.created_at, first.created_at);

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);
  }

  #[test]
  fn set_env_vars_roundtrips_and_updates_timestamp() {
    let conn = test_conn();
    let project = create(&conn, "Overnight", "/repo/overnight").unwrap();
    assert_eq!(project.env_vars, Vec::<EnvVar>::new());

    let vars = vec![
      EnvVar { key: "API_URL".to_string(), value: "https://example.com".to_string() },
      EnvVar { key: "DEBUG".to_string(), value: "1".to_string() },
    ];
    let updated = set_env_vars(&conn, &project.id, &vars).unwrap();
    assert_eq!(updated.env_vars, vars);
    assert!(updated.updated_at >= project.updated_at);
    assert_eq!(get(&conn, &project.id).unwrap().env_vars, vars);
  }

  #[test]
  fn set_env_vars_missing_id_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(set_env_vars(&conn, "missing", &[]), Err(Error::NotFound)));
  }

  #[test]
  fn set_secrets_roundtrips_and_updates_timestamp() {
    let conn = test_conn();
    let project = create(&conn, "Overnight", "/repo/overnight").unwrap();
    assert_eq!(project.secrets, Vec::<Secret>::new());

    let secrets = vec![
      Secret {
        target: SecretTarget::Custom { env: "GITHUB_TOKEN".to_string(), hosts: vec!["api.github.com".to_string()], placeholder: None },
        source: SecretSource::Value { value: "ghp_abc".to_string() },
      },
      Secret {
        target: SecretTarget::Custom { env: "NPM_TOKEN".to_string(), hosts: vec!["registry.npmjs.org".to_string()], placeholder: None },
        source: SecretSource::Value { value: "npm_xyz".to_string() },
      },
    ];
    let updated = set_secrets(&conn, &project.id, &secrets).unwrap();
    assert_eq!(updated.secrets, secrets);
    assert!(updated.updated_at >= project.updated_at);
    assert_eq!(get(&conn, &project.id).unwrap().secrets, secrets);
  }

  #[test]
  fn set_secrets_missing_id_returns_not_found() {
    let conn = test_conn();
    assert!(matches!(set_secrets(&conn, "missing", &[]), Err(Error::NotFound)));
  }

  #[test]
  fn deleting_project_nulls_task_project_id() {
    let conn = test_conn();
    let project = create(&conn, "Overnight", "/repo/overnight").unwrap();
    let task = crate::db::tasks::create(&conn, "Do work", None, None, "todo", None).unwrap();
    conn
      .execute(
        "UPDATE tasks SET project_id = ?1 WHERE id = ?2",
        params![project.id, task.id],
      )
      .unwrap();

    delete(&conn, &project.id).unwrap();

    let task = crate::db::tasks::get(&conn, &task.id).unwrap();
    assert_eq!(task.project_id, None);
  }
}
