use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::daemon_log;
use crate::db::error::{Error, Result};
use crate::db::models::{
  CommandLogEntry, ContainerMetric, EnvVar, HostMetric, JiraIssue, Project, Sandbox, SandboxBackup, Secret, SecretSource,
  SecretTarget, Session, Task,
};
use crate::db::{
  backups, command_log, container_metrics, host_metrics, jira_issues, metrics, projects, sandboxes, sessions,
  settings, tasks, DbPool,
};
use crate::jira::{self, JiraClient};
use crate::providers::claude_code::ClaudeCodeProvider;
use crate::providers::AgentProvider;

#[tauri::command]
pub fn list_tasks(pool: State<DbPool>) -> Result<Vec<Task>> {
  let conn = pool.get()?;
  tasks::list(&conn)
}

#[tauri::command]
pub fn create_task(
  pool: State<DbPool>,
  title: String,
  description: Option<String>,
  jira_key: Option<String>,
) -> Result<Task> {
  let conn = pool.get()?;
  tasks::create(
    &conn,
    &title,
    description.as_deref(),
    jira_key.as_deref(),
    "todo",
    None,
  )
}

#[tauri::command]
pub fn get_task(pool: State<DbPool>, id: String) -> Result<Task> {
  let conn = pool.get()?;
  tasks::get(&conn, &id)
}

#[tauri::command]
pub fn update_task_status(pool: State<DbPool>, id: String, status: String) -> Result<Task> {
  let conn = pool.get()?;
  tasks::update_status(&conn, &id, &status)
}

#[tauri::command]
pub fn delete_task(pool: State<DbPool>, id: String) -> Result<()> {
  let conn = pool.get()?;
  tasks::delete(&conn, &id)
}

#[tauri::command]
pub fn start_session(
  pool: State<DbPool>,
  task_id: String,
  agent_provider: String,
  mode: String,
) -> Result<Session> {
  let conn = pool.get()?;
  sessions::create(&conn, &task_id, &agent_provider, &mode, None)
}

#[tauri::command]
pub fn get_session(pool: State<DbPool>, id: String) -> Result<Session> {
  let conn = pool.get()?;
  sessions::get(&conn, &id)
}

#[tauri::command]
pub fn list_sessions_for_task(pool: State<DbPool>, task_id: String) -> Result<Vec<Session>> {
  let conn = pool.get()?;
  sessions::list_for_task(&conn, &task_id)
}

#[tauri::command]
pub fn end_session(pool: State<DbPool>, id: String, status: String) -> Result<Session> {
  let conn = pool.get()?;
  sessions::end(&conn, &id, &status)
}

// No chat UI consumes this yet — it exists so OVN-19/OVN-53 can be
// smoke-tested end-to-end via devtools (`invoke`) before the UI lands.
// Errors are stringified (rather than using `db::error::Error`/`Result`)
// since they can originate from `providers::Error` too, matching the
// existing pattern used by the Jira commands below.
#[tauri::command]
pub fn start_plan_session(
  app: AppHandle,
  pool: State<DbPool>,
  task_id: String,
  prompt: String,
) -> std::result::Result<Session, String> {
  let pool = pool.inner().clone();
  let provider = ClaudeCodeProvider;

  let handle = provider.launch_plan_session(&app, &pool, &task_id, &prompt).map_err(|e| e.to_string())?;

  let session = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sessions::get(&conn, &handle.session_id).map_err(|e| e.to_string())?
  };

  let bg_app = app.clone();
  let bg_pool = pool.clone();
  tauri::async_runtime::spawn(async move {
    use futures::StreamExt;
    let provider = ClaudeCodeProvider;
    let mut events = provider.stream_events(&bg_app, &bg_pool, &handle);
    while events.next().await.is_some() {}
  });

  Ok(session)
}

fn validate_repo_path(repo_path: &str) -> Result<()> {
  if !std::path::Path::new(repo_path).join(".git").exists() {
    return Err(Error::InvalidRepoPath(repo_path.to_string()));
  }
  Ok(())
}

#[tauri::command]
pub fn list_projects(pool: State<DbPool>) -> Result<Vec<Project>> {
  let conn = pool.get()?;
  projects::list(&conn)
}

#[tauri::command]
pub fn create_project(
  pool: State<DbPool>,
  name: String,
  repo_path: String,
  plans_path: Option<String>,
  dev_server_port: Option<i64>,
) -> Result<Project> {
  validate_repo_path(&repo_path)?;
  let conn = pool.get()?;
  projects::create(&conn, &name, &repo_path, plans_path.as_deref(), dev_server_port)
}

#[tauri::command]
pub fn update_project(
  pool: State<DbPool>,
  id: String,
  name: String,
  repo_path: String,
  plans_path: Option<String>,
  dev_server_port: Option<i64>,
) -> Result<Project> {
  validate_repo_path(&repo_path)?;
  let conn = pool.get()?;
  projects::update(&conn, &id, &name, &repo_path, plans_path.as_deref(), dev_server_port)
}

#[tauri::command]
pub fn delete_project(pool: State<DbPool>, id: String) -> Result<()> {
  let conn = pool.get()?;
  projects::delete(&conn, &id)
}

const SKILL_FOLDERS_KEY: &str = "skill_folders";
const GLOBAL_ENV_VARS_KEY: &str = "global_env_vars";
const GLOBAL_SECRETS_KEY: &str = "global_secrets";

/// Every service identifier `sbx secret set` recognizes (from the `sbx
/// secret set` CLI reference's "Available services" line). A service
/// secret's `target.service` must be one of these — `sbx` interprets
/// anything else as invalid.
const KNOWN_SECRET_SERVICES: &[&str] = &[
  "anthropic", "copilot", "cursor", "devin", "droid", "github", "google", "groq", "mistral", "nebius", "openai", "openrouter", "xai",
];

/// Env var names must be valid POSIX identifiers, and no two entries in
/// one list may share a key — the UI enforces this too, but the backend
/// is the source of truth since these values get written into a shell
/// file (see sbx::set_env_vars).
fn validate_env_vars(vars: &[EnvVar]) -> std::result::Result<(), String> {
  let mut seen = std::collections::HashSet::new();
  for var in vars {
    if !is_valid_identifier(&var.key) {
      return Err(format!(
        "invalid environment variable name: \"{}\" (must start with a letter or underscore and contain only letters, digits, and underscores)",
        var.key
      ));
    }
    if !seen.insert(var.key.clone()) {
      return Err(format!("duplicate environment variable: {}", var.key));
    }
  }
  Ok(())
}

fn is_valid_identifier(s: &str) -> bool {
  let mut chars = s.chars();
  let first_ok = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_');
  first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Validates one secret's shape, independent of scope. A custom
/// secret's `env` becomes a real env var name inside the sandbox (see
/// `secrets_as_env_vars`), so it follows the same identifier rule as
/// `validate_env_vars`; its `hosts` must be non-empty, with no entry
/// containing whitespace. A service secret's name must be one `sbx`
/// actually recognizes. A custom secret's `source` must carry a
/// non-empty value/reference/command — a service secret's literal value
/// may be empty (see `register_secrets`'s doc comment on why that skips
/// `sbx` instead of sending an empty value), but a reference or command
/// still can't be, for either target, since there's nothing to resolve.
/// No two entries in one list may share a `key()`.
fn validate_secrets(secrets: &[Secret]) -> std::result::Result<(), String> {
  let mut seen = std::collections::HashSet::new();
  for secret in secrets {
    match &secret.target {
      SecretTarget::Service { service } => {
        if !KNOWN_SECRET_SERVICES.contains(&service.as_str()) {
          return Err(format!("unknown secret service: \"{service}\" (must be one of {KNOWN_SECRET_SERVICES:?})"));
        }
      }
      SecretTarget::Custom { env, hosts, .. } => {
        if !is_valid_identifier(env) {
          return Err(format!(
            "invalid secret env var name: \"{env}\" (must start with a letter or underscore and contain only letters, digits, and underscores)"
          ));
        }
        if hosts.is_empty() || hosts.iter().any(|h| h.is_empty() || h.chars().any(|c| c.is_whitespace())) {
          return Err(format!("secret \"{env}\" needs at least one host, with no whitespace"));
        }
      }
    }
    match (&secret.target, &secret.source) {
      (SecretTarget::Custom { .. }, SecretSource::Value { value }) if value.is_empty() => {
        return Err(format!("secret \"{}\": value can't be empty", secret.key()))
      }
      (_, SecretSource::Reference { reference, .. }) if reference.is_empty() => {
        return Err(format!("secret \"{}\": reference can't be empty", secret.key()))
      }
      (_, SecretSource::Command { command, .. }) if command.is_empty() => {
        return Err(format!("secret \"{}\": command can't be empty", secret.key()))
      }
      _ => {}
    }
    if !seen.insert(secret.key().to_string()) {
      return Err(format!("duplicate secret: {}", secret.key()));
    }
  }
  Ok(())
}

#[tauri::command]
pub fn get_global_env_vars(pool: State<DbPool>) -> std::result::Result<Vec<EnvVar>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(settings::get_json(&conn, GLOBAL_ENV_VARS_KEY).map_err(|e| e.to_string())?.unwrap_or_default())
}

#[tauri::command]
pub async fn save_global_env_vars(app: AppHandle, pool: State<'_, DbPool>, vars: Vec<EnvVar>) -> std::result::Result<(), String> {
  validate_env_vars(&vars)?;
  let pool = pool.inner().clone();
  {
    let conn = pool.get().map_err(|e| e.to_string())?;
    settings::set_json(&conn, GLOBAL_ENV_VARS_KEY, &vars).map_err(|e| e.to_string())?;
  }
  let all_sandboxes = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::list(&conn).map_err(|e| e.to_string())?
  };
  push_env_vars_to_running_sandboxes(&app, &pool, &all_sandboxes).await;
  Ok(())
}

#[tauri::command]
pub fn get_project_env_vars(pool: State<DbPool>, project_id: String) -> std::result::Result<Vec<EnvVar>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(projects::get(&conn, &project_id).map_err(|e| e.to_string())?.env_vars)
}

#[tauri::command]
pub async fn save_project_env_vars(
  app: AppHandle,
  pool: State<'_, DbPool>,
  project_id: String,
  vars: Vec<EnvVar>,
) -> std::result::Result<Project, String> {
  validate_env_vars(&vars)?;
  let pool = pool.inner().clone();
  let project = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    projects::set_env_vars(&conn, &project_id, &vars).map_err(|e| e.to_string())?
  };
  let project_sandboxes = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::list_for_project(&conn, &project_id).map_err(|e| e.to_string())?
  };
  push_env_vars_to_running_sandboxes(&app, &pool, &project_sandboxes).await;
  Ok(project)
}

#[tauri::command]
pub fn get_sandbox_env_vars(pool: State<DbPool>, id: String) -> std::result::Result<Vec<EnvVar>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(sandboxes::get(&conn, &id).map_err(|e| e.to_string())?.env_vars)
}

#[tauri::command]
pub async fn save_sandbox_env_vars(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
  vars: Vec<EnvVar>,
) -> std::result::Result<Sandbox, String> {
  validate_env_vars(&vars)?;
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::set_env_vars(&conn, &id, &vars).map_err(|e| e.to_string())?
  };
  // A still-"starting" sandbox has no sbx_name yet — provision_sandbox
  // will push the merged list once it's ready, so there's nothing to push
  // right now.
  if let Some(name) = &sandbox.sbx_name {
    let merged = full_env_vars_for_sandbox(&pool, &sandbox.project_id, &sandbox)?;
    crate::sbx::set_env_vars(&app, name, &merged).await.map_err(|e| e.to_string())?;
  }
  Ok(sandbox)
}

/// Best-effort live push of every env var that belongs in a sandbox
/// right now — its own plain env vars plus every custom secret's
/// placeholder (see `full_env_vars_for_sandbox`) — into every currently
/// running sandbox in `targets`. Called after a global or project-scoped
/// env var *or secret* list changes, so already-running sandboxes pick
/// up the change immediately instead of only sandboxes created
/// afterward. A sandbox that isn't running yet is skipped rather than
/// failed — it'll get the current list from `provision_sandbox` (on
/// create) or the next live edit anyway. One sandbox's push failing
/// doesn't stop the others, mirroring `sync_git_identity`'s best-effort
/// philosophy.
async fn push_env_vars_to_running_sandboxes(app: &AppHandle, pool: &DbPool, targets: &[Sandbox]) {
  for sandbox in targets {
    if sandbox.status != "running" {
      continue;
    }
    let Some(name) = &sandbox.sbx_name else { continue };
    let merged = match full_env_vars_for_sandbox(pool, &sandbox.project_id, sandbox) {
      Ok(merged) => merged,
      Err(e) => {
        log::warn!("push_env_vars_to_running_sandboxes: failed to compute env vars for {}: {e}", sandbox.id);
        continue;
      }
    };
    if let Err(e) = crate::sbx::set_env_vars(app, name, &merged).await {
      log::warn!("push_env_vars_to_running_sandboxes: failed to push env vars to {name}: {e}");
    }
  }
}

/// Merges global, project, and sandbox-scoped env vars into the single
/// list actually pushed into a sandbox — sandbox-scoped wins over
/// project-scoped, which wins over global, on a key collision.
fn merged_env_vars(pool: &DbPool, project_id: &str, sandbox_vars: &[EnvVar]) -> std::result::Result<Vec<EnvVar>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let global: Vec<EnvVar> = settings::get_json(&conn, GLOBAL_ENV_VARS_KEY).map_err(|e| e.to_string())?.unwrap_or_default();
  let project = projects::get(&conn, project_id).map_err(|e| e.to_string())?.env_vars;
  let mut merged: std::collections::HashMap<String, String> = std::collections::HashMap::new();
  for var in global.into_iter().chain(project).chain(sandbox_vars.iter().cloned()) {
    merged.insert(var.key, var.value);
  }
  let mut result: Vec<EnvVar> = merged.into_iter().map(|(key, value)| EnvVar { key, value }).collect();
  result.sort_by(|a, b| a.key.cmp(&b.key));
  Ok(result)
}

#[tauri::command]
pub fn get_global_secrets(pool: State<DbPool>) -> std::result::Result<Vec<Secret>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  // Tolerates a value stored in an older shape (e.g. from before `Secret`
  // gained `target`/`source`) as if nothing were stored at all, rather
  // than failing outright — the same resilience `row_to_project`/
  // `row_to_sandbox` already give `env_vars`/`secrets` read off a row.
  Ok(settings::get_json(&conn, GLOBAL_SECRETS_KEY).ok().flatten().unwrap_or_default())
}

#[tauri::command]
pub async fn save_global_secrets(app: AppHandle, pool: State<'_, DbPool>, secrets: Vec<Secret>) -> std::result::Result<(), String> {
  validate_secrets(&secrets)?;
  let pool = pool.inner().clone();
  let previous: Vec<Secret> = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    // See get_global_secrets's doc comment on why a parse failure here
    // degrades to empty instead of failing the save outright.
    settings::get_json(&conn, GLOBAL_SECRETS_KEY).ok().flatten().unwrap_or_default()
  };
  // Drop what this save removed before re-setting what it kept: a secret
  // this list no longer carries has to leave sbx's store too, not just
  // ours.
  let resolved = resolve_secret_placeholders(&previous, &secrets);
  let removed = removed_secrets(&previous, &resolved);
  remove_secrets_from_sbx(&app, &removed, None).await;
  // The native global form (`sbx secret set-custom`, no --sandbox) is the
  // primary effect of this save, so unlike the "also push live to other
  // already-running sandboxes" step below, a failure here must reach the
  // caller rather than being logged and swallowed.
  register_secrets(&app, &resolved, None).await?;
  {
    let conn = pool.get().map_err(|e| e.to_string())?;
    settings::set_json(&conn, GLOBAL_SECRETS_KEY, &resolved).map_err(|e| e.to_string())?;
  }
  // Proxy awareness is automatic for a global secret, but exporting a
  // custom secret's placeholder as an actual env var inside a sandbox is
  // not — push the combined env var list to every running sandbox so an
  // edit here takes effect immediately, same UX as env vars.
  let all_sandboxes = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::list(&conn).map_err(|e| e.to_string())?
  };
  push_env_vars_to_running_sandboxes(&app, &pool, &all_sandboxes).await;
  Ok(())
}

#[tauri::command]
pub fn get_project_secrets(pool: State<DbPool>, project_id: String) -> std::result::Result<Vec<Secret>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(projects::get(&conn, &project_id).map_err(|e| e.to_string())?.secrets)
}

#[tauri::command]
pub async fn save_project_secrets(
  app: AppHandle,
  pool: State<'_, DbPool>,
  project_id: String,
  secrets: Vec<Secret>,
) -> std::result::Result<Project, String> {
  validate_secrets(&secrets)?;
  let pool = pool.inner().clone();
  let previous = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    projects::get(&conn, &project_id).map_err(|e| e.to_string())?.secrets
  };
  // Resolved once at the project level — every sandbox in the project
  // registers against this same list, so they all export the same
  // placeholder for the same project secret (see
  // resolve_secret_placeholders's doc comment).
  let resolved = resolve_secret_placeholders(&previous, &secrets);
  let project = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    projects::set_secrets(&conn, &project_id, &resolved).map_err(|e| e.to_string())?
  };
  let removed = removed_secrets(&previous, &resolved);
  let project_sandboxes = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::list_for_project(&conn, &project_id).map_err(|e| e.to_string())?
  };
  // Project scope has no native registration in sbx — a project secret
  // only ever reaches sbx via a --sandbox-scoped call to each sandbox in
  // the project. Register/remove against each currently-running one
  // (a sandbox not yet running gets the current list from
  // provision_sandbox on create, or the next live edit) before pushing
  // the combined env var list below.
  for sandbox in project_sandboxes.iter().filter(|s| s.status == "running" && s.sbx_name.is_some()) {
    let name = sandbox.sbx_name.as_deref().unwrap();
    remove_secrets_from_sbx(&app, &removed, Some(name)).await;
    register_secrets(&app, &resolved, Some(name)).await?;
  }
  push_env_vars_to_running_sandboxes(&app, &pool, &project_sandboxes).await;
  Ok(project)
}

#[tauri::command]
pub fn get_sandbox_secrets(pool: State<DbPool>, id: String) -> std::result::Result<Vec<Secret>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(sandboxes::get(&conn, &id).map_err(|e| e.to_string())?.secrets)
}

#[tauri::command]
pub async fn save_sandbox_secrets(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
  secrets: Vec<Secret>,
) -> std::result::Result<Sandbox, String> {
  validate_secrets(&secrets)?;
  let pool = pool.inner().clone();
  let previous = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?.secrets
  };
  let resolved = resolve_secret_placeholders(&previous, &secrets);
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::set_secrets(&conn, &id, &resolved).map_err(|e| e.to_string())?
  };
  // Same "nothing to push yet" case as save_sandbox_env_vars: a
  // still-"starting" sandbox has no sbx_name, provision_sandbox registers
  // and pushes once it's ready.
  if let Some(name) = &sandbox.sbx_name {
    remove_secrets_from_sbx(&app, &removed_secrets(&previous, &sandbox.secrets), Some(name)).await;
    // Direct target of this save — a registration failure must reach
    // the caller, same reasoning as save_sandbox_env_vars's
    // crate::sbx::set_env_vars(...).map_err(...)? below it.
    register_secrets(&app, &sandbox.secrets, Some(name)).await?;
    let merged = full_env_vars_for_sandbox(&pool, &sandbox.project_id, &sandbox)?;
    crate::sbx::set_env_vars(&app, name, &merged).await.map_err(|e| e.to_string())?;
  }
  Ok(sandbox)
}

/// The entries of `previous` whose `key()` no longer appears in
/// `current` — what a save just dropped, and therefore what has to be
/// removed from sbx's own store rather than merely disappearing from
/// ours.
fn removed_secrets(previous: &[Secret], current: &[Secret]) -> Vec<Secret> {
  let kept: std::collections::HashSet<&str> = current.iter().map(|s| s.key()).collect();
  previous.iter().filter(|s| !kept.contains(s.key())).cloned().collect()
}

/// Removes every entry in `removed` from sbx's own store at the given
/// scope. Best-effort by design, unlike `reconcile_secrets`: the
/// likeliest failure is "no such secret" — a registration that never
/// landed, or an entry already removed by hand — which leaves the store
/// in exactly the state the caller asked for, and shouldn't fail the
/// save that dropped it.
async fn remove_secrets_from_sbx(app: &AppHandle, removed: &[Secret], sandbox_name: Option<&str>) {
  for secret in removed {
    let result = match &secret.target {
      SecretTarget::Service { service } => crate::sbx::remove_service_secret(app, service, sandbox_name).await,
      SecretTarget::Custom { hosts, .. } => {
        let mut last = Ok(());
        for host in hosts {
          last = crate::sbx::remove_custom_secret(app, host, sandbox_name).await;
        }
        last
      }
    };
    if let Err(e) = result {
      log::warn!("remove_secrets_from_sbx: failed to remove secret {}: {e}", secret.key());
    }
  }
}

/// Merges global, project, and sandbox-scoped secrets into the single
/// list actually registered for a sandbox — sandbox-scoped wins over
/// project-scoped, which wins over global, on a `key()` collision (the
/// whole entry is replaced, not merged field by field). Mirrors
/// `merged_env_vars` exactly, just keyed by `Secret::key()` instead of a
/// plain string field.
fn merged_secrets(pool: &DbPool, project_id: &str, sandbox_secrets: &[Secret]) -> std::result::Result<Vec<Secret>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  // See get_global_secrets's doc comment on why a parse failure here
  // degrades to empty instead of failing outright.
  let global: Vec<Secret> = settings::get_json(&conn, GLOBAL_SECRETS_KEY).ok().flatten().unwrap_or_default();
  let project = projects::get(&conn, project_id).map_err(|e| e.to_string())?.secrets;
  let mut merged: std::collections::HashMap<String, Secret> = std::collections::HashMap::new();
  for secret in global.into_iter().chain(project).chain(sandbox_secrets.iter().cloned()) {
    merged.insert(secret.key().to_string(), secret);
  }
  let mut result: Vec<Secret> = merged.into_values().collect();
  result.sort_by(|a, b| a.key().cmp(b.key()));
  Ok(result)
}

/// The `EnvVar`s a list of secrets contributes to a sandbox's actual
/// environment — a custom secret's placeholder, exported under its own
/// `env` name, so a process inside the sandbox that reads that variable
/// sends the placeholder and the proxy swaps in the real value on the
/// way out. A service secret contributes nothing here: it's consumed by
/// the sandbox's own agent/kit bootstrap, not by an env var this app
/// manages. A custom secret with no placeholder yet is skipped — that
/// only happens for an entry `reconcile_secrets` hasn't processed yet.
fn secrets_as_env_vars(secrets: &[Secret]) -> Vec<EnvVar> {
  secrets
    .iter()
    .filter_map(|s| match &s.target {
      SecretTarget::Custom { env, placeholder: Some(p), .. } => Some(EnvVar { key: env.clone(), value: p.clone() }),
      _ => None,
    })
    .collect()
}

/// The full list of env vars that belong in one sandbox right now: its
/// plain env vars (see `merged_env_vars`) plus the placeholder for every
/// custom secret that applies to it. Every "push env vars to a sandbox"
/// call site uses this instead of `merged_env_vars` alone — `set_env_vars`
/// rewrites its whole managed block on each call, so pushing env vars and
/// secret placeholders through two separate calls would have each
/// overwrite the other's contribution.
fn full_env_vars_for_sandbox(pool: &DbPool, project_id: &str, sandbox: &Sandbox) -> std::result::Result<Vec<EnvVar>, String> {
  let mut vars = merged_env_vars(pool, project_id, &sandbox.env_vars)?;
  vars.extend(secrets_as_env_vars(&merged_secrets(pool, project_id, &sandbox.secrets)?));
  Ok(vars)
}

/// Generates a fresh, stable placeholder for a brand-new custom secret.
/// Never reused across secrets, never regenerated for an existing one —
/// see `resolve_secret_placeholders`.
fn generate_placeholder() -> String {
  format!("sbx-cs-{}", uuid::Uuid::new_v4().simple())
}

/// Resolves the placeholder for every entry in `incoming`, given what
/// was previously saved at this scope. A custom secret's placeholder is
/// tied to its identity (the env var name), not its content: if
/// `previous` has any entry with the same `key()`, its placeholder is
/// reused even if the value or hosts changed — a sandbox that already
/// has the old placeholder exported keeps working without needing to be
/// re-synced. Only a `key()` with no match in `previous` at all gets a
/// freshly generated placeholder. A service secret is returned
/// unchanged — it carries no placeholder.
///
/// Pure and side-effect-free: this only decides *what* to register, not
/// *where* — see `register_secrets` for the sbx calls, which run once
/// per target (once for a global/sandbox-scoped save, once per affected
/// sandbox for a project-scoped one) against this same resolved list.
fn resolve_secret_placeholders(previous: &[Secret], incoming: &[Secret]) -> Vec<Secret> {
  incoming
    .iter()
    .map(|secret| match &secret.target {
      SecretTarget::Service { .. } => secret.clone(),
      SecretTarget::Custom { env, hosts, placeholder } => {
        let resolved = placeholder
          .clone()
          .or_else(|| {
            previous.iter().find(|p| p.key() == secret.key()).and_then(|p| match &p.target {
              SecretTarget::Custom { placeholder, .. } => placeholder.clone(),
              SecretTarget::Service { .. } => None,
            })
          })
          .unwrap_or_else(generate_placeholder);
        Secret {
          target: SecretTarget::Custom { env: env.clone(), hosts: hosts.clone(), placeholder: Some(resolved) },
          source: secret.source.clone(),
        }
      }
    })
    .collect()
}

/// Registers every secret in `secrets` (already placeholder-resolved by
/// `resolve_secret_placeholders`) at the given scope — the direct effect
/// of a save action: global, one sandbox, or one of a project's
/// sandboxes. Always (re-)registers the whole list rather than only
/// what changed since a prior save: every call is idempotent (the
/// placeholder is fixed ahead of time, never regenerated here), and a
/// secrets save is an infrequent user action, not a hot path, so this
/// trades a handful of redundant CLI calls for not having to track "did
/// this specific entry change at this specific target" across scopes
/// that don't share one registration. A failure is propagated, not
/// swallowed — this is the direct, single-target effect of the caller's
/// save action.
async fn register_secrets(app: &AppHandle, secrets: &[Secret], sandbox_name: Option<&str>) -> std::result::Result<(), String> {
  for secret in secrets {
    match &secret.target {
      SecretTarget::Service { service } => {
        // An empty literal value means the service's credential is
        // managed some other way (already set directly via `sbx`,
        // OAuth, etc.) and this entry exists only so the app can track
        // and remove it — skip sbx entirely rather than sending an
        // empty value, which would make `sbx secret set` fall back to
        // an interactive prompt this app can't answer (see
        // SBX_SECRET_COMMAND_TIMEOUT's doc comment for what that leads
        // to). A reference or command source is never empty here —
        // validate_secrets rejects that regardless of target.
        if matches!(&secret.source, SecretSource::Value { value } if value.is_empty()) {
          continue;
        }
        crate::sbx::set_service_secret(app, service, &secret.source, sandbox_name).await.map_err(|e| e.to_string())?;
      }
      SecretTarget::Custom { env, hosts, placeholder } => {
        let placeholder = placeholder.as_deref().ok_or_else(|| format!("secret {env} has no placeholder to register"))?;
        crate::sbx::set_custom_secret(app, env, hosts, placeholder, &secret.source, sandbox_name)
          .await
          .map_err(|e| e.to_string())?;
      }
    }
  }
  Ok(())
}

/// A single shared setting, not one per agent — its meaning is always
/// relative to whichever agent `SANDBOX_AGENT_KEY` currently names. Changing
/// the default agent (see `save_default_agent`) doesn't rewrite this value;
/// `get_settings`/`create_sandbox` fall back to the *new* agent's own
/// `AgentKit::default_permission_mode` whenever this stored value would be
/// invalid for it, so a leftover Claude-shaped string never leaks into a
/// Codex sandbox's defaults or vice versa.
const DEFAULT_PERMISSION_MODE_KEY: &str = "default_permission_mode";

#[derive(Serialize)]
pub struct AppSettings {
  pub default_permission_mode: String,
  pub skill_folders: Vec<String>,
}

#[tauri::command]
pub fn get_settings(pool: State<DbPool>) -> Result<AppSettings> {
  let conn = pool.get()?;
  let default_agent = settings::get(&conn, SANDBOX_AGENT_KEY)?.unwrap_or_else(|| DEFAULT_AGENT.to_string());
  let agent_kit = crate::agents::get(&default_agent).unwrap_or(&crate::agents::CLAUDE);
  Ok(AppSettings {
    default_permission_mode: settings::get(&conn, DEFAULT_PERMISSION_MODE_KEY)?
      .unwrap_or_else(|| agent_kit.default_permission_mode.to_string()),
    skill_folders: settings::get_json(&conn, SKILL_FOLDERS_KEY)?.unwrap_or_default(),
  })
}

fn validate_permission_mode(kit: &crate::agents::AgentKit, mode: &str) -> Result<()> {
  if !kit.permission_modes.contains(&mode) {
    return Err(Error::InvalidValue(format!("invalid {} permission mode: {mode}", kit.label)));
  }
  Ok(())
}

/// `agent` is the agent this `default_permission_mode` was chosen for —
/// passed explicitly by the frontend (its currently-selected default agent
/// at save time) rather than read back from `SANDBOX_AGENT_KEY`, so saving
/// both settings together in one "Save" click never depends on which of
/// the two writes lands first.
#[tauri::command]
pub fn save_settings(pool: State<DbPool>, agent: String, default_permission_mode: String) -> Result<()> {
  let agent_kit = crate::agents::get(&agent).ok_or_else(|| Error::InvalidValue(format!("invalid agent: {agent}")))?;
  validate_permission_mode(agent_kit, &default_permission_mode)?;
  let conn = pool.get()?;
  settings::set(&conn, DEFAULT_PERMISSION_MODE_KEY, &default_permission_mode)
}

/// Persists the chosen skill folders and copies each into sbx's shared
/// agent-skills store (a plain host-filesystem copy — no sandbox involved).
#[tauri::command]
pub fn save_skill_folders(pool: State<DbPool>, folders: Vec<String>) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set_json(&conn, SKILL_FOLDERS_KEY, &folders).map_err(|e| e.to_string())?;

  let store_dir = crate::skills::skill_store_dir();
  std::fs::create_dir_all(&store_dir).map_err(|e| e.to_string())?;
  for folder in &folders {
    crate::skills::copy_folder_into_store(&store_dir, std::path::Path::new(folder)).map_err(|e| e.to_string())?;
  }
  Ok(())
}

/// The frontend has no other way to tell Windows apart from macOS/Linux
/// (no `@tauri-apps/plugin-os` or similar dependency) — used to gate the
/// Windows-only terminal-host popover in `open_sandbox_terminal`'s UI.
#[tauri::command]
pub fn get_platform() -> String {
  std::env::consts::OS.to_string()
}

const SANDBOX_TERMINAL_HOST_KEY: &str = "default_terminal_host";
const DEFAULT_TERMINAL_HOST: &str = "cmd";
const VALID_TERMINAL_HOSTS: [&str; 2] = ["cmd", "powershell"];

/// Windows-only: which console app wraps `sbx exec -it <name> bash` when a
/// per-launch choice isn't given (see `open_sandbox_terminal`). Ignored on
/// macOS/Linux, but kept unscoped by OS here — same shape as
/// `DEFAULT_PERMISSION_MODE_KEY` — since it's a harmless no-op elsewhere.
#[tauri::command]
pub fn get_default_terminal_host(pool: State<DbPool>) -> std::result::Result<String, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(
    settings::get(&conn, SANDBOX_TERMINAL_HOST_KEY)
      .map_err(|e| e.to_string())?
      .unwrap_or_else(|| DEFAULT_TERMINAL_HOST.to_string()),
  )
}

#[tauri::command]
pub fn save_default_terminal_host(pool: State<DbPool>, terminal_host: String) -> std::result::Result<(), String> {
  if !VALID_TERMINAL_HOSTS.contains(&terminal_host.as_str()) {
    return Err(format!("invalid terminal host: {terminal_host}"));
  }
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, SANDBOX_TERMINAL_HOST_KEY, &terminal_host).map_err(|e| e.to_string())
}

const LAYOUT_EXPANDED_KEY: &str = "layout_expanded";

#[tauri::command]
pub fn get_layout_expanded(pool: State<DbPool>) -> std::result::Result<bool, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(
    settings::get(&conn, LAYOUT_EXPANDED_KEY)
      .map_err(|e| e.to_string())?
      .map(|v| v == "true")
      .unwrap_or(false),
  )
}

#[tauri::command]
pub fn save_layout_expanded(pool: State<DbPool>, expanded: bool) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, LAYOUT_EXPANDED_KEY, if expanded { "true" } else { "false" }).map_err(|e| e.to_string())
}

const SANDBOX_AGENT_KEY: &str = "default_agent";
const DEFAULT_AGENT: &str = "claude";

fn agent_cli_token(agent: &str) -> Option<&'static str> {
  crate::agents::get(agent).map(|kit| kit.cli_token)
}

/// The app-wide default agent a new sandbox is created with when the
/// creation dialog doesn't override it. See `create_sandbox`.
fn resolve_default_agent(pool: &DbPool) -> std::result::Result<String, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(
    settings::get(&conn, SANDBOX_AGENT_KEY)
      .map_err(|e| e.to_string())?
      .unwrap_or_else(|| DEFAULT_AGENT.to_string()),
  )
}

#[tauri::command]
pub fn get_default_agent(pool: State<DbPool>) -> std::result::Result<String, String> {
  resolve_default_agent(pool.inner())
}

#[tauri::command]
pub fn save_default_agent(pool: State<DbPool>, agent: String) -> std::result::Result<(), String> {
  if agent_cli_token(&agent).is_none() {
    return Err(format!("invalid agent: {agent}"));
  }
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, SANDBOX_AGENT_KEY, &agent).map_err(|e| e.to_string())
}

/// One agent's public-facing config — the wire form of `agents::AgentKit`,
/// trimming fields (`cli_token`, `permission_flag`, `home_dir`,
/// `host_auth_relative_path`) that are only meaningful to backend-side
/// sandbox orchestration and have no frontend use today.
#[derive(Serialize)]
pub struct AgentInfo {
  pub id: String,
  pub label: String,
  pub permission_modes: Vec<String>,
  pub default_permission_mode: String,
  pub secret_service: String,
}

/// Every known agent's display info, straight from `agents::all()` — lets
/// the frontend eventually stop hand-mirroring `AGENTS`/`AGENT_LABELS`/
/// `PERMISSION_MODES` as parallel, comment-synced constants, though nothing
/// consumes this yet (those constants remain the frontend's actual source
/// of truth for now, since they double as compile-time-checked `Agent`
/// union members). Exists so a new `AgentKit` is visible from the wire the
/// moment it's added, without waiting on that frontend migration.
#[tauri::command]
pub fn list_agents() -> Vec<AgentInfo> {
  crate::agents::all()
    .iter()
    .map(|kit| AgentInfo {
      id: kit.id.to_string(),
      label: kit.label.to_string(),
      permission_modes: kit.permission_modes.iter().map(|m| m.to_string()).collect(),
      default_permission_mode: kit.default_permission_mode.to_string(),
      secret_service: kit.secret_service.to_string(),
    })
    .collect()
}

const SIDEBAR_WIDTH_KEY: &str = "sidebar_width";
const DEFAULT_SIDEBAR_WIDTH: i32 = 208;
const MIN_SIDEBAR_WIDTH: i32 = 208;
const MAX_SIDEBAR_WIDTH: i32 = 360;

#[tauri::command]
pub fn get_sidebar_width(pool: State<DbPool>) -> std::result::Result<i32, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(
    settings::get(&conn, SIDEBAR_WIDTH_KEY)
      .map_err(|e| e.to_string())?
      .and_then(|s| s.parse::<i32>().ok())
      .unwrap_or(DEFAULT_SIDEBAR_WIDTH),
  )
}

#[tauri::command]
pub fn save_sidebar_width(pool: State<DbPool>, width: i32) -> std::result::Result<(), String> {
  let clamped = width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, SIDEBAR_WIDTH_KEY, &clamped.to_string()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod sidebar_width_tests {
  use super::*;

  #[test]
  fn clamps_below_min() {
    assert_eq!(150.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH), MIN_SIDEBAR_WIDTH);
  }

  #[test]
  fn clamps_above_max() {
    assert_eq!(500.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH), MAX_SIDEBAR_WIDTH);
  }

  #[test]
  fn keeps_value_in_range() {
    assert_eq!(300.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH), 300);
  }
}

#[cfg(test)]
mod agent_cli_token_tests {
  use super::*;

  #[test]
  fn known_agents_return_their_tokens() {
    assert_eq!(agent_cli_token("claude"), Some("claude"));
    assert_eq!(agent_cli_token("codex"), Some("codex"));
  }

  #[test]
  fn unknown_agent_returns_none() {
    assert_eq!(agent_cli_token("gpt4"), None);
  }

  #[test]
  fn list_agents_includes_every_known_agent_with_its_secret_service() {
    let agents = list_agents();
    let claude = agents.iter().find(|a| a.id == "claude").expect("claude listed");
    assert_eq!(claude.secret_service, "anthropic");
    assert_eq!(claude.default_permission_mode, "default");
    let codex = agents.iter().find(|a| a.id == "codex").expect("codex listed");
    assert_eq!(codex.secret_service, "openai");
    assert_eq!(codex.default_permission_mode, "never");
  }
}

const DAEMON_LOG_PATH_KEY: &str = "daemon_log_path";

/// Developer > Telemetry page. Falls back to `daemon_log::default_path()`
/// (an OS-specific guess, not a confirmed real location) until the user
/// saves their own path.
#[tauri::command]
pub fn get_daemon_log_path(pool: State<DbPool>) -> std::result::Result<String, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(settings::get(&conn, DAEMON_LOG_PATH_KEY).map_err(|e| e.to_string())?.unwrap_or_else(daemon_log::default_path))
}

#[tauri::command]
pub fn save_daemon_log_path(pool: State<DbPool>, path: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, DAEMON_LOG_PATH_KEY, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_daemon_log(path: String) -> std::result::Result<daemon_log::DaemonLogResult, String> {
  daemon_log::read_tail(&path).map_err(|e| e.to_string())
}

const JIRA_SITE_KEY: &str = "jira_site";
const JIRA_EMAIL_KEY: &str = "jira_email";
const JIRA_JQL_KEY: &str = "jira_jql";
const DEFAULT_JIRA_JQL: &str = "assignee = currentUser() ORDER BY updated DESC";

#[derive(Serialize)]
pub struct JiraConfig {
  pub site: Option<String>,
  pub email: Option<String>,
  pub jql: Option<String>,
  pub has_token: bool,
}

#[tauri::command]
pub fn save_jira_config(
  pool: State<DbPool>,
  site: String,
  email: String,
  jql: Option<String>,
  api_token: String,
) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let jql = jql.filter(|j| !j.trim().is_empty()).unwrap_or_else(|| DEFAULT_JIRA_JQL.to_string());
  settings::set(&conn, JIRA_SITE_KEY, &site).map_err(|e| e.to_string())?;
  settings::set(&conn, JIRA_EMAIL_KEY, &email).map_err(|e| e.to_string())?;
  settings::set(&conn, JIRA_JQL_KEY, &jql).map_err(|e| e.to_string())?;
  jira::credentials::store_token(&api_token).map_err(|e| e.to_string())?;
  Ok(())
}

#[tauri::command]
pub fn get_jira_config(pool: State<DbPool>) -> std::result::Result<JiraConfig, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(JiraConfig {
    site: settings::get(&conn, JIRA_SITE_KEY).map_err(|e| e.to_string())?,
    email: settings::get(&conn, JIRA_EMAIL_KEY).map_err(|e| e.to_string())?,
    jql: settings::get(&conn, JIRA_JQL_KEY).map_err(|e| e.to_string())?,
    has_token: jira::credentials::has_token(),
  })
}

#[tauri::command]
pub async fn sync_jira_issues(pool: State<'_, DbPool>) -> std::result::Result<usize, String> {
  let (site, email, jql) = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let site = settings::get(&conn, JIRA_SITE_KEY)
      .map_err(|e| e.to_string())?
      .ok_or_else(|| jira::Error::ConfigMissing("site").to_string())?;
    let email = settings::get(&conn, JIRA_EMAIL_KEY)
      .map_err(|e| e.to_string())?
      .ok_or_else(|| jira::Error::ConfigMissing("email").to_string())?;
    let jql = settings::get(&conn, JIRA_JQL_KEY)
      .map_err(|e| e.to_string())?
      .unwrap_or_else(|| DEFAULT_JIRA_JQL.to_string());
    (site, email, jql)
  };
  let token = jira::credentials::get_token().map_err(|e| e.to_string())?;

  let client = JiraClient::new(site, email, token);
  let issues = client.search_all(&jql).await.map_err(|e| e.to_string())?;

  let mut conn = pool.get().map_err(|e| e.to_string())?;
  jira_issues::upsert_many(&mut conn, &issues).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_jira_issues(pool: State<DbPool>) -> std::result::Result<Vec<JiraIssue>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  jira_issues::list(&conn).map_err(|e| e.to_string())
}

// Sandboxes ------------------------------------------------------------
//
// A sandbox is an `sbx` (Docker Sandboxes) microVM, bound either directly
// to a project's repo_path ("mount" mode) or to an in-VM clone of it
// ("clone" mode, via `sbx create --clone` — sbx manages the clone itself,
// we don't). Errors are stringified rather than using `db::error::Error`/
// `Result` since they can originate from `sbx::Error` too, matching the
// existing pattern used by the Jira commands above.

const SANDBOX_MEMORY_MB: u32 = 2048;
const SANDBOX_PORT: u16 = 8080;
const SANDBOX_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const SANDBOX_READY_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

fn check_free_memory() -> std::result::Result<(), String> {
  let free_mb = crate::sbx::host_free_memory_mb();
  if free_mb < SANDBOX_MEMORY_MB as f64 {
    return Err(format!(
      "not enough free memory to start a sandbox: {free_mb:.0}MB free, {SANDBOX_MEMORY_MB}MB recommended"
    ));
  }
  Ok(())
}

/// Sanitizes a user-provided sandbox name for `sbx create --name`:
/// whitespace becomes `-`, everything else unsafe for a CLI/DNS-ish
/// identifier is dropped.
fn sanitize_sbx_name(name: &str) -> String {
  name
    .trim()
    .chars()
    .map(|c| if c.is_whitespace() { '-' } else { c })
    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
    .collect()
}

/// Base name to try first: the sanitized user-provided name, or
/// `<project-name>-<4-char-id>` (from `sandbox_id`) when none was given.
fn base_sbx_name(user_name: Option<&str>, project_name: &str, sandbox_id: &str) -> String {
  match user_name.map(str::trim).filter(|n| !n.is_empty()) {
    Some(n) => sanitize_sbx_name(n),
    None => {
      let suffix: String = sandbox_id.chars().filter(char::is_ascii_alphanumeric).take(4).collect();
      format!("{}-{suffix}", sanitize_sbx_name(project_name))
    }
  }
}

/// `sbx create --name <name>` fails with "... already exists, use sbx run
/// --name ... to connect" on collision (see `sbx::resume`'s doc comment).
fn is_name_conflict(err: &str) -> bool {
  err.contains("already exists")
}

const MAX_NAME_ATTEMPTS: u32 = 50;

/// Finds a unique sbx sandbox name by calling `try_create` with `base`,
/// then `base-2`, `base-3`, ... on each name-conflict error, until it
/// succeeds or fails for an unrelated reason. Leans on `sbx`'s own
/// uniqueness enforcement rather than pre-checking a name list — TOCTOU-safe
/// by construction, since the name is only "taken" once `sbx` itself accepts
/// it.
async fn resolve_unique_sbx_name<F, Fut>(base: &str, mut try_create: F) -> std::result::Result<String, String>
where
  F: FnMut(String) -> Fut,
  Fut: std::future::Future<Output = std::result::Result<(), String>>,
{
  for attempt in 1..=MAX_NAME_ATTEMPTS {
    let candidate = if attempt == 1 { base.to_string() } else { format!("{base}-{attempt}") };
    match try_create(candidate.clone()).await {
      Ok(()) => return Ok(candidate),
      Err(e) if is_name_conflict(&e) => continue,
      Err(e) => return Err(e),
    }
  }
  Err(format!("could not find a free sandbox name based on {base:?} after {MAX_NAME_ATTEMPTS} attempts"))
}

#[tauri::command]
pub async fn sbx_health_check(app: AppHandle) -> std::result::Result<(), String> {
  crate::sbx::health_check(&app).await.map_err(|e| e.to_string())
}

const VALID_NETWORK_POLICY_PRESETS: [&str; 3] = ["allow-all", "balanced", "deny-all"];
const NETWORK_POLICY_PRESET_KEY: &str = "default_network_policy_preset";
const VALID_SANDBOX_NETWORK_PRESETS: [&str; 2] = ["open", "locked-down"];
const VALID_NETWORK_DECISIONS: [&str; 2] = ["allow", "deny"];

/// One-time, machine-wide setup answering sbx's interactive network-policy
/// prompt headlessly. The frontend calls this when `create_sandbox` fails
/// with the "network policy hasn't been initialized" error. Also records
/// the chosen preset locally (see `record_network_policy_preset`) so it
/// stays in sync with whatever the Settings screen's "Network Policy"
/// card shows as current, regardless of which flow initialized it.
#[tauri::command]
pub async fn init_sbx_policy(app: AppHandle, pool: State<'_, DbPool>, preset: String) -> std::result::Result<(), String> {
  if !VALID_NETWORK_POLICY_PRESETS.contains(&preset.as_str()) {
    return Err(format!("invalid network policy preset: {preset}"));
  }
  crate::sbx::policy_init(&app, &preset).await.map_err(|e| e.to_string())?;
  record_network_policy_preset(pool.inner(), &preset)
}

fn record_network_policy_preset(pool: &DbPool, preset: &str) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, NETWORK_POLICY_PRESET_KEY, preset).map_err(|e| e.to_string())
}

/// Everything the Settings screen's "Network Policy" card needs in one
/// call: the machine-wide preset last applied via `init_sbx_policy`/
/// `save_default_network_policy_preset` (`None` if this machine's policy
/// has never been initialized — sbx itself has no documented "what preset
/// is currently active" query, so this is our own record of the choice,
/// not a mirror of sbx's rule state), and the live custom allow/deny rule
/// list, always read straight from sbx (see `sbx::policy_list`).
#[derive(Serialize)]
pub struct NetworkPolicySettings {
  pub preset: Option<String>,
  pub rules: Vec<crate::sbx::PolicyRule>,
}

#[tauri::command]
pub async fn get_network_policy_settings(
  app: AppHandle,
  pool: State<'_, DbPool>,
) -> std::result::Result<NetworkPolicySettings, String> {
  let preset = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    settings::get(&conn, NETWORK_POLICY_PRESET_KEY).map_err(|e| e.to_string())?
  };
  let rules = crate::sbx::policy_list(&app, None).await.map_err(|e| e.to_string())?;
  Ok(NetworkPolicySettings { preset, rules })
}

/// Changing an already-initialized preset needs `sbx policy reset` first —
/// `sbx policy init` on its own only works the very first time (confirmed
/// against a real `sbx` install: it otherwise errors with "global network
/// policy is already initialized"). The frontend must have already
/// confirmed this with the user before calling this command, since the
/// reset it falls back to stops every currently running sandbox.
#[tauri::command]
pub async fn save_default_network_policy_preset(
  app: AppHandle,
  pool: State<'_, DbPool>,
  preset: String,
) -> std::result::Result<(), String> {
  if !VALID_NETWORK_POLICY_PRESETS.contains(&preset.as_str()) {
    return Err(format!("invalid network policy preset: {preset}"));
  }
  match crate::sbx::policy_init(&app, &preset).await {
    Ok(()) => {}
    Err(crate::sbx::Error::PolicyAlreadyInitialized) => {
      crate::sbx::policy_reset(&app).await.map_err(|e| e.to_string())?;
      // The reset just stopped every sandbox on the machine out from under
      // the app (see sandboxes::stop_all_for_policy_reset) — reflect that
      // before re-initializing, so the UI doesn't keep showing them as running.
      let conn = pool.get().map_err(|e| e.to_string())?;
      sandboxes::stop_all_for_policy_reset(&conn).map_err(|e| e.to_string())?;
      drop(conn);
      crate::sbx::policy_init(&app, &preset).await.map_err(|e| e.to_string())?;
    }
    Err(e) => return Err(e.to_string()),
  }
  record_network_policy_preset(pool.inner(), &preset)
}

fn validate_network_decision(decision: &str) -> std::result::Result<(), String> {
  if !VALID_NETWORK_DECISIONS.contains(&decision) {
    return Err(format!("invalid network rule decision: {decision} (expected \"allow\" or \"deny\")"));
  }
  Ok(())
}

/// Adds a machine-wide custom allow/deny rule directly via `sbx`. No DB
/// write — per Branch 3's design, custom rule lists are never mirrored
/// locally; `get_network_policy_settings`/`sbx::policy_list` always read
/// them back live to avoid drift against sbx's real rule store.
#[tauri::command]
pub async fn add_network_rule(app: AppHandle, decision: String, host: String) -> std::result::Result<(), String> {
  validate_network_decision(&decision)?;
  match decision.as_str() {
    "allow" => crate::sbx::policy_allow(&app, None, &host).await,
    _ => crate::sbx::policy_deny(&app, None, &host).await,
  }
  .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_network_rule(app: AppHandle, host: String) -> std::result::Result<(), String> {
  crate::sbx::policy_rm(&app, None, &host).await.map_err(|e| e.to_string())
}

/// Loads the sandbox's `sbx_name` — every per-sandbox network command
/// needs it to scope its `sbx policy ... --sandbox <name>` call, and none
/// of them make sense before the sandbox has one (i.e. before its first
/// `sbx create` succeeds).
fn require_sbx_name(pool: &DbPool, id: &str) -> std::result::Result<String, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::get(&conn, id)
    .map_err(|e| e.to_string())?
    .sbx_name
    .ok_or_else(|| "sandbox has no sbx sandbox yet".to_string())
}

#[tauri::command]
pub async fn get_sandbox_network_rules(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
) -> std::result::Result<Vec<crate::sbx::PolicyRule>, String> {
  let name = require_sbx_name(pool.inner(), &id)?;
  crate::sbx::policy_list(&app, Some(&name)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_sandbox_network_rule(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
  decision: String,
  host: String,
) -> std::result::Result<(), String> {
  validate_network_decision(&decision)?;
  let name = require_sbx_name(pool.inner(), &id)?;
  match decision.as_str() {
    "allow" => crate::sbx::policy_allow(&app, Some(&name), &host).await,
    _ => crate::sbx::policy_deny(&app, Some(&name), &host).await,
  }
  .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_sandbox_network_rule(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
  host: String,
) -> std::result::Result<(), String> {
  let name = require_sbx_name(pool.inner(), &id)?;
  crate::sbx::policy_rm(&app, Some(&name), &host).await.map_err(|e| e.to_string())
}

/// Applies (or clears) a per-sandbox network preset override. Unlike the
/// machine-wide preset (a whole rule-set swap via `sbx policy init`),
/// there's no documented way to scope a whole preset to one sandbox — the
/// only single-sandbox lever `sbx` exposes is a wildcard allow/deny rule,
/// so "Open"/"Locked Down" are implemented as `sbx policy allow/deny
/// --sandbox <name> "**"` and clearing removes that same wildcard rule.
/// There's deliberately no per-sandbox "Balanced" option for this reason
/// (see the plan's Branch 3 assumptions).
#[tauri::command]
pub async fn set_sandbox_network_preset_override(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
  preset: Option<String>,
) -> std::result::Result<Sandbox, String> {
  if let Some(preset) = &preset {
    if !VALID_SANDBOX_NETWORK_PRESETS.contains(&preset.as_str()) {
      return Err(format!(
        "invalid sandbox network preset override: {preset} (expected \"open\" or \"locked-down\")"
      ));
    }
  }
  let name = require_sbx_name(pool.inner(), &id)?;
  match preset.as_deref() {
    Some("open") => crate::sbx::policy_allow(&app, Some(&name), "**").await,
    Some(_) => crate::sbx::policy_deny(&app, Some(&name), "**").await,
    None => crate::sbx::policy_rm(&app, Some(&name), "**").await,
  }
  .map_err(|e| e.to_string())?;

  // A "Locked Down" override denies all network for this one sandbox,
  // which can stop the sandbox itself as a side effect (confirmed against
  // a real sbx install) — with no dedicated event to react to, so
  // reconcile the DB with sbx's live status before recording the override.
  // Best-effort: a failed status read shouldn't block recording the
  // override the user actually asked for.
  if let Ok(Some(status)) = crate::sbx::current_status(&app, &name).await {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::update_status(&conn, &id, &status, None, None).map_err(|e| e.to_string())?;
  }

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::set_network_preset_override(&conn, &id, preset.as_deref()).map_err(|e| e.to_string())
}

/// Stores the user's Anthropic API key as a global sbx secret so Claude
/// Code sessions inside every sandbox authenticate automatically instead
/// of needing an interactive `/login`.
#[tauri::command]
pub async fn set_anthropic_api_key(app: AppHandle, token: String) -> std::result::Result<(), String> {
  crate::sbx::set_anthropic_secret(&app, &token).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sandboxes(pool: State<'_, DbPool>) -> std::result::Result<Vec<Sandbox>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::list(&conn).map_err(|e| e.to_string())
}

/// True when `repo_path` (a project's host-side checkout, host-native form)
/// and `workspace_path` (an `sbx ls` row, already POSIX-normalized/`~`
/// -expanded by `sbx::list_all`) refer to the same directory — a
/// mount-mode-style match, same idea as `create_sandbox` binding a
/// mount-mode sandbox straight to `project.repo_path`. Normalizes
/// `repo_path` through the same drive-letter conversion so this still
/// matches on a Windows host, and ignores a trailing slash either side.
fn repo_path_matches_workspace(repo_path: &str, workspace_path: &str) -> bool {
  fn normalize(path: &str) -> String {
    crate::sbx::windows_path_to_posix(path).trim_end_matches('/').to_string()
  }
  !repo_path.is_empty() && normalize(repo_path) == normalize(workspace_path)
}

/// Finds sandboxes `sbx ls` knows about with no matching `sandboxes` row
/// (by `sbx_name`) and creates one for each — so sandboxes created or left
/// behind outside the app still show up and can be managed. Deliberately
/// one-directional: never touches an existing DB row whose sandbox has
/// disappeared from `sbx ls` (a different, out-of-scope feature). Polled
/// periodically by the frontend (every 4th sandbox-poll tick), not on
/// every tick, since it shells out to `sbx ls` itself.
#[tauri::command]
pub async fn adopt_orphan_sandboxes(app: AppHandle, pool: State<'_, DbPool>) -> std::result::Result<Vec<Sandbox>, String> {
  let rows = crate::sbx::list_all(&app).await.map_err(|e| e.to_string())?;
  adopt_orphan_sandboxes_from_rows(pool.inner(), rows)
}

/// Core of `adopt_orphan_sandboxes`, decoupled from `AppHandle` (mirrors
/// `stop_sandbox_with`'s split) so it's testable against a real DB pool
/// with hand-built `SbxListRow`s, without a real `sbx` install.
fn adopt_orphan_sandboxes_from_rows(pool: &DbPool, rows: Vec<crate::sbx::SbxListRow>) -> std::result::Result<Vec<Sandbox>, String> {
  let mut adopted = Vec::new();

  for row in rows {
    let conn = pool.get().map_err(|e| e.to_string())?;
    if sandboxes::find_by_sbx_name(&conn, &row.sbx_name).map_err(|e| e.to_string())?.is_some() {
      continue;
    }

    let known_projects = projects::list(&conn).map_err(|e| e.to_string())?;
    let matched_project = row
      .workspace_path
      .as_deref()
      .and_then(|workspace| known_projects.iter().find(|p| repo_path_matches_workspace(&p.repo_path, workspace)));

    // Matched: bind it as a mount-mode sandbox against the project's own
    // repo_path, same shape create_sandbox uses for mount mode. No match
    // (e.g. a clone-mode sandbox, whose clone lives inside the VM with no
    // host-visible path) — fall back to the lazily created "Unassigned"
    // project rather than a dangling/sentinel project id.
    let (project_id, mode, folder_path) = match matched_project {
      Some(project) => (project.id.clone(), "mount", Some(project.repo_path.clone())),
      None => {
        let unassigned = match projects::get_or_create_unassigned(&conn) {
          Ok(project) => project,
          Err(e) => {
            log::warn!("adopt_orphan_sandboxes: could not resolve Unassigned project for {}: {e}", row.sbx_name);
            continue;
          }
        };
        (unassigned.id, "clone", None)
      }
    };

    // Insert via the normal creation path (status "starting", no
    // sbx_name), then immediately finalize with what sbx ls actually
    // reported — mirrors create_sandbox's create-then-update_status shape,
    // and lets the DB's per-project uniqueness constraints still apply. A
    // constraint hit here (e.g. a real active mount sandbox already
    // tracked for the matched project) skips this one row rather than
    // failing the whole adoption pass.
    // base_branch = NULL: an adopted sandbox's host checkout state at the
    // time `sbx create` actually ran (outside the app) is unknown to us.
    let created = match sandboxes::create(&conn, &project_id, mode, folder_path.as_deref(), None, crate::agents::CLAUDE.default_permission_mode, None, "claude") {
      Ok(sandbox) => sandbox,
      Err(e) => {
        log::warn!("adopt_orphan_sandboxes: could not create a row for {}: {e}", row.sbx_name);
        continue;
      }
    };

    match sandboxes::update_status(&conn, &created.id, &row.status, Some(&row.sbx_name), None) {
      Ok(updated) => adopted.push(updated),
      Err(e) => log::warn!("adopt_orphan_sandboxes: could not finalize adopted row for {}: {e}", row.sbx_name),
    }
  }

  Ok(adopted)
}

#[cfg(test)]
mod adopt_orphan_sandboxes_tests {
  use super::*;
  use crate::sbx::SbxListRow;
  use r2d2_sqlite::SqliteConnectionManager;

  fn test_pool() -> DbPool {
    let path = std::env::temp_dir().join(format!("overnight-adopt-orphan-{}", uuid::Uuid::new_v4()));
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    crate::db::migrations::migrations().to_latest(&mut conn).unwrap();
    drop(conn);
    let manager = SqliteConnectionManager::file(&path).with_init(|conn| conn.execute_batch("PRAGMA foreign_keys = ON;"));
    r2d2::Pool::builder().max_size(2).build(manager).unwrap()
  }

  fn row(sbx_name: &str, status: &str, workspace_path: Option<&str>) -> SbxListRow {
    SbxListRow { sbx_name: sbx_name.to_string(), status: status.to_string(), workspace_path: workspace_path.map(String::from) }
  }

  #[test]
  fn matches_repo_path_ignoring_trailing_slash() {
    assert!(repo_path_matches_workspace("/repo/overnight", "/repo/overnight"));
    assert!(repo_path_matches_workspace("/repo/overnight/", "/repo/overnight"));
    assert!(!repo_path_matches_workspace("/repo/other", "/repo/overnight"));
    assert!(!repo_path_matches_workspace("", "/repo/overnight"));
  }

  #[test]
  fn adopts_sandbox_matching_a_known_project_as_mount_mode() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    let project = projects::create(&conn, "Overnight", "/repo/overnight", None, None).unwrap();
    drop(conn);

    let adopted =
      adopt_orphan_sandboxes_from_rows(&pool, vec![row("found-sandbox", "running", Some("/repo/overnight"))]).unwrap();

    assert_eq!(adopted.len(), 1);
    assert_eq!(adopted[0].project_id, project.id);
    assert_eq!(adopted[0].mode, "mount");
    assert_eq!(adopted[0].folder_path.as_deref(), Some("/repo/overnight"));
    assert_eq!(adopted[0].status, "running");
    assert_eq!(adopted[0].sbx_name.as_deref(), Some("found-sandbox"));
    assert_eq!(adopted[0].permission_mode, crate::agents::CLAUDE.default_permission_mode);
  }

  #[test]
  fn adopts_unmatched_sandbox_into_unassigned_project_as_clone_mode() {
    let pool = test_pool();

    let adopted =
      adopt_orphan_sandboxes_from_rows(&pool, vec![row("clone-sandbox", "stopped", None)]).unwrap();

    assert_eq!(adopted.len(), 1);
    assert_eq!(adopted[0].project_id, crate::db::projects::UNASSIGNED_PROJECT_ID);
    assert_eq!(adopted[0].mode, "clone");
    assert_eq!(adopted[0].folder_path, None);
    assert_eq!(adopted[0].status, "stopped");

    let conn = pool.get().unwrap();
    let unassigned = projects::list(&conn).unwrap();
    assert_eq!(unassigned.len(), 1);
    assert_eq!(unassigned[0].id, crate::db::projects::UNASSIGNED_PROJECT_ID);
  }

  #[test]
  fn skips_sandboxes_already_known_by_sbx_name() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    let project = projects::create(&conn, "Overnight", "/repo/overnight", None, None).unwrap();
    let existing = sandboxes::create(&conn, &project.id, "mount", None, None, "default", None, "claude").unwrap();
    sandboxes::update_status(&conn, &existing.id, "running", Some("already-known"), None).unwrap();
    drop(conn);

    let adopted =
      adopt_orphan_sandboxes_from_rows(&pool, vec![row("already-known", "running", Some("/repo/overnight"))]).unwrap();

    assert!(adopted.is_empty());
    let conn = pool.get().unwrap();
    assert_eq!(sandboxes::list(&conn).unwrap().len(), 1);
  }

  #[test]
  fn one_bad_row_does_not_block_others() {
    let pool = test_pool();

    let adopted = adopt_orphan_sandboxes_from_rows(
      &pool,
      vec![row("first-sandbox", "running", None), row("second-sandbox", "stopped", None)],
    )
    .unwrap();

    assert_eq!(adopted.len(), 2);
    let names: Vec<_> = adopted.iter().filter_map(|s| s.sbx_name.clone()).collect();
    assert_eq!(names, vec!["first-sandbox", "second-sandbox"]);
  }
}

#[tauri::command]
pub async fn create_sandbox(
  app: AppHandle,
  pool: State<'_, DbPool>,
  project_id: String,
  mode: String,
  name: Option<String>,
  permission_mode: Option<String>,
  agent: Option<String>,
) -> std::result::Result<Sandbox, String> {
  if mode != "mount" && mode != "clone" {
    return Err(format!("invalid sandbox mode: {mode} (expected \"mount\" or \"clone\")"));
  }
  let name = name.filter(|n| !n.trim().is_empty());
  let pool = pool.inner().clone();

  let stored_default_agent = resolve_default_agent(&pool)?;
  let agent = match agent.filter(|a| !a.trim().is_empty()) {
    Some(a) => a,
    None => stored_default_agent.clone(),
  };
  let agent_kit = crate::agents::get(&agent).ok_or_else(|| format!("invalid agent: {agent}"))?;

  // Unset means "use this agent's configured default" — resolved and
  // snapshotted onto the sandbox now rather than looked up again on every
  // session launch, same as folder_path/sbx_name are fixed at creation
  // time. The single `default_permission_mode` setting is only consulted
  // when `agent` matches the app's current default agent — it was saved
  // for that agent specifically (see `DEFAULT_PERMISSION_MODE_KEY`'s doc
  // comment), so applying it to a different, explicitly-overridden agent
  // could hand it a permission-mode string that isn't even valid for it.
  let permission_mode = match permission_mode.filter(|m| !m.trim().is_empty()) {
    Some(m) => {
      if !agent_kit.permission_modes.contains(&m.as_str()) {
        return Err(format!("invalid permission mode: {m}"));
      }
      m
    }
    None if agent == stored_default_agent => {
      let conn = pool.get().map_err(|e| e.to_string())?;
      settings::get(&conn, DEFAULT_PERMISSION_MODE_KEY)
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| agent_kit.default_permission_mode.to_string())
    }
    None => agent_kit.default_permission_mode.to_string(),
  };

  let project = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    projects::get(&conn, &project_id).map_err(|e| e.to_string())?
  };

  {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let existing = sandboxes::list_for_project(&conn, &project_id).map_err(|e| e.to_string())?;
    if mode == "mount" && existing.iter().any(|s| s.mode == "mount" && matches!(s.status.as_str(), "starting" | "running" | "stopping")) {
      return Err("a mount-mode sandbox is already running for this project".to_string());
    }
    if existing.iter().any(|s| s.status == "starting") {
      return Err("a sandbox is already starting for this project".to_string());
    }
  }

  check_free_memory()?;

  // Snapshotted once, up front, the same way permission_mode/mode already
  // are — `None` on detached HEAD or a non-git repo_path, surfaced by the
  // Branch tab as "created before branch tracking was added" rather than a
  // guessed fallback.
  let base_branch = crate::git::current_branch(&project.repo_path);

  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    // Clone mode's clone lives inside the sandbox VM, not on the host — no
    // host-visible folder_path to record for it.
    let initial_folder = if mode == "mount" { Some(project.repo_path.as_str()) } else { None };
    sandboxes::create(&conn, &project_id, &mode, initial_folder, name.as_deref(), &permission_mode, base_branch.as_deref(), &agent)
      .map_err(|e| e.to_string())?
  };

  // From here on the sandbox row already exists (status "starting"). If
  // anything below fails — including the very first `sbx create` on this
  // machine hanging on an unanswered interactive network-policy prompt —
  // mark the row "error" instead of leaving it stuck at "starting" forever
  // with no signal that it didn't work.
  match provision_sandbox(&app, &pool, &project, &sandbox, &mode).await {
    Ok(result) => Ok(result),
    Err(e) => {
      if let Ok(conn) = pool.get() {
        let _ = sandboxes::update_status(&conn, &sandbox.id, "error", None, None);
      }
      Err(e)
    }
  }
}

async fn provision_sandbox(
  app: &AppHandle,
  pool: &DbPool,
  project: &Project,
  sandbox: &Sandbox,
  mode: &str,
) -> std::result::Result<Sandbox, String> {
  let agent_kit = crate::agents::get(&sandbox.agent).ok_or_else(|| format!("unknown agent: {}", sandbox.agent))?;
  let base_name = base_sbx_name(sandbox.name.as_deref(), &project.name, &sandbox.id);
  let clone = mode == "clone";
  let name = resolve_unique_sbx_name(&base_name, |candidate| async move {
    crate::sbx::create(app, &candidate, clone, &project.repo_path, agent_kit.cli_token).await.map_err(|e| e.to_string())
  })
  .await?;
  // `create` returning doesn't guarantee the VM is actually up for `exec`
  // yet — wait before the exec-dependent steps below.
  crate::sbx::wait_until_ready(app, &name, SANDBOX_READY_TIMEOUT, SANDBOX_READY_POLL_INTERVAL)
    .await
    .map_err(|e| e.to_string())?;

  // Independent once the sandbox is ready: git identity, the
  // permission-mode alias, env vars, secrets, port publishing/lookup, and
  // (best-effort) syncing this agent's host auth file in. Run concurrently.
  let (permission_result, host_port_result, env_result, secret_result, (), ()) = tokio::join!(
    crate::sbx::set_default_permission_mode(app, &name, agent_kit.cli_token, agent_kit.permission_flag, &sandbox.permission_mode),
    async {
      crate::sbx::publish_port(app, &name, SANDBOX_PORT).await?;
      crate::sbx::host_port(app, &name, SANDBOX_PORT).await
    },
    async {
      let merged = full_env_vars_for_sandbox(pool, &project.id, sandbox)?;
      crate::sbx::set_env_vars(app, &name, &merged).await.map_err(|e| e.to_string())
    },
    async {
      // Already fully placeholder-resolved: a brand-new sandbox's own
      // secrets are always empty, so this is just the already-persisted
      // global/project lists — only registering them against this new
      // sandbox is needed, not resolving anything new.
      let merged = merged_secrets(pool, &project.id, &sandbox.secrets)?;
      register_secrets(app, &merged, Some(&name)).await
    },
    sync_git_identity(app, &name),
    sync_host_agent_auth(app, &name, agent_kit),
  );
  permission_result.map_err(|e| e.to_string())?;
  let host_port = host_port_result.map_err(|e| e.to_string())?;
  env_result?;
  secret_result?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, &sandbox.id, "running", Some(&name), host_port.map(i64::from))
    .map_err(|e| e.to_string())
}

/// Pushes the host's global git identity into a freshly created sandbox.
/// `sbx` never imports host `$HOME` config, so a fresh sandbox otherwise
/// has none. Best-effort: no host identity, or any push failure, is
/// silently skipped rather than failing sandbox creation.
async fn sync_git_identity(app: &AppHandle, name: &str) {
  for key in ["user.name", "user.email"] {
    if let Some(value) = host_git_config(key) {
      let _ = crate::sbx::set_git_config(app, name, key, &value).await;
    }
  }
}

fn host_git_config(key: &str) -> Option<String> {
  let mut cmd = std::process::Command::new("git");
  cmd.args(["config", "--global", key]);
  crate::git::hide_console(&mut cmd);
  let output = cmd.output().ok()?;
  if !output.status.success() {
    return None;
  }
  let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
  (!value.is_empty()).then_some(value)
}

/// Best-effort: copies this agent's host auth file (if it has one — see
/// `AgentKit::host_auth_relative_path`) into the sandbox, so e.g. a Codex
/// session logged into on the host via `codex login` doesn't need a fresh
/// interactive login inside every sandbox. Claude Code has no entry here
/// (`host_auth_relative_path: None`) — it authenticates purely through the
/// `anthropic` secret pushed by `register_secrets` instead. A missing host
/// file (never logged in yet) or a failed copy is silently skipped, same
/// as `sync_git_identity`'s "no host identity is fine" behavior.
async fn sync_host_agent_auth(app: &AppHandle, name: &str, kit: &crate::agents::AgentKit) {
  let Some(relative) = kit.host_auth_relative_path else { return };
  let Some(home) = host_home_dir() else { return };
  let host_path = std::path::Path::new(&home).join(relative);
  if !host_path.is_file() {
    return;
  }
  let remote_path = host_auth_remote_path(kit.home_dir, relative);
  if let Err(e) = crate::sbx::cp_to_sandbox(app, name, &host_path.to_string_lossy(), &remote_path, None).await {
    log::warn!("sync_host_agent_auth: failed to copy {} into {name}: {e}", host_path.display());
  }
}

/// Where `sync_host_agent_auth` copies `relative`'s host file to inside the
/// sandbox: `home_dir` joined with just the file's basename, not the whole
/// `relative` path — `relative` may itself start with a directory that
/// duplicates part of `home_dir` (e.g. Codex's `.codex/auth.json` against
/// `home_dir = "/home/agent/.codex"`), and joining the full relative path
/// would produce `/home/agent/.codex/.codex/auth.json` instead of the
/// `/home/agent/.codex/auth.json` the agent actually reads.
fn host_auth_remote_path(home_dir: &str, relative: &str) -> String {
  let basename = std::path::Path::new(relative).file_name().and_then(|f| f.to_str()).unwrap_or(relative);
  format!("{home_dir}/{basename}")
}

fn host_home_dir() -> Option<String> {
  #[cfg(target_os = "windows")]
  {
    std::env::var("USERPROFILE").ok()
  }
  #[cfg(not(target_os = "windows"))]
  {
    std::env::var("HOME").ok()
  }
}

#[cfg(test)]
mod host_auth_remote_path_tests {
  use super::host_auth_remote_path;

  #[test]
  fn joins_home_dir_with_just_the_basename_not_the_full_relative_path() {
    assert_eq!(host_auth_remote_path("/home/agent/.codex", ".codex/auth.json"), "/home/agent/.codex/auth.json");
  }

  #[test]
  fn a_bare_filename_with_no_directory_component_still_joins_correctly() {
    assert_eq!(host_auth_remote_path("/home/agent/.claude", "credentials.json"), "/home/agent/.claude/credentials.json");
  }
}

#[cfg(test)]
mod sbx_naming_tests {
  use super::{base_sbx_name, resolve_unique_sbx_name};
  use std::sync::Mutex;

  #[test]
  fn sanitizes_whitespace_and_unsafe_characters() {
    assert_eq!(base_sbx_name(Some("my sandbox!!"), "overnight", "irrelevant"), "my-sandbox");
    assert_eq!(base_sbx_name(Some("  tabs\tand\nnewlines  "), "overnight", "irrelevant"), "tabs-and-newlines");
  }

  #[test]
  fn falls_back_to_project_name_and_4_char_id() {
    assert_eq!(base_sbx_name(None, "Overnight", "ab12-cd34"), "Overnight-ab12");
    assert_eq!(base_sbx_name(Some("   "), "Overnight", "ab12-cd34"), "Overnight-ab12");
  }

  #[tokio::test]
  async fn succeeds_immediately_when_name_is_free() {
    let name = resolve_unique_sbx_name("my-sandbox", |candidate| async move {
      assert_eq!(candidate, "my-sandbox");
      Ok(())
    })
    .await
    .unwrap();
    assert_eq!(name, "my-sandbox");
  }

  #[tokio::test]
  async fn retries_with_bumped_suffix_on_name_conflict() {
    let taken = Mutex::new(vec!["my-sandbox".to_string(), "my-sandbox-2".to_string()]);
    let name = resolve_unique_sbx_name("my-sandbox", |candidate| {
      let is_taken = taken.lock().unwrap().contains(&candidate);
      async move {
        if is_taken {
          Err("sandbox \"x\" already exists, use sbx run --name x to connect".to_string())
        } else {
          Ok(())
        }
      }
    })
    .await
    .unwrap();
    assert_eq!(name, "my-sandbox-3");
  }

  #[tokio::test]
  async fn propagates_unrelated_errors_without_retrying() {
    let err = resolve_unique_sbx_name("my-sandbox", |_candidate| async move { Err("network policy not set".to_string()) })
      .await
      .unwrap_err();
    assert_eq!(err, "network policy not set");
  }
}

/// Core of `stop_sandbox`, decoupled from `AppHandle` so `stop` is
/// mockable in tests. Marks the row "stopping" before awaiting `stop` (so
/// pollers see the transition immediately, not only once the — possibly
/// slow — teardown finishes), then "stopped" once it resolves.
async fn stop_sandbox_with<F, Fut>(pool: &DbPool, id: &str, stop: F) -> std::result::Result<Sandbox, String>
where
  F: FnOnce() -> Fut,
  Fut: std::future::Future<Output = crate::sbx::Result<()>>,
{
  {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::update_status(&conn, id, "stopping", None, None).map_err(|e| e.to_string())?;
  }
  stop().await.map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, id, "stopped", None, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let pool = pool.inner().clone();
  let name = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id)
      .map_err(|e| e.to_string())?
      .sbx_name
      .ok_or_else(|| "sandbox has no sbx sandbox to stop".to_string())?
  };
  stop_sandbox_with(&pool, &id, || async {
    // Best-effort, right before the sandbox actually goes offline: a
    // failed read here (e.g. a non-git workspace, or the sandbox already
    // being unresponsive) must never block the stop itself — log and move
    // on to the real `sbx stop` regardless.
    if let Err(e) = capture_branch_snapshot(&app, &pool, &id, &name).await {
      log::warn!("stop_sandbox: best-effort branch snapshot failed for {id}: {e}");
    }
    crate::sbx::stop(&app, &name).await
  })
  .await
}

#[cfg(test)]
mod stop_sandbox_tests {
  use super::*;
  use r2d2_sqlite::SqliteConnectionManager;

  fn test_pool() -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("overnight-stop-sandbox-{}", uuid::Uuid::new_v4()));
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    crate::db::migrations::migrations().to_latest(&mut conn).unwrap();
    drop(conn);
    let pool = r2d2::Pool::builder().max_size(2).build(SqliteConnectionManager::file(&path)).unwrap();
    (pool, path)
  }

  #[tokio::test]
  async fn sets_stopping_before_stop_resolves_then_stopped() {
    let (pool, path) = test_pool();
    let sandbox_id = {
      let conn = pool.get().unwrap();
      let project = projects::create(&conn, "Overnight", "/repo", None, None).unwrap();
      sandboxes::create(&conn, &project.id, "mount", None, None, "default", None, "claude").unwrap().id
    };

    let check_pool = pool.clone();
    let check_id = sandbox_id.clone();
    let result = stop_sandbox_with(&pool, &sandbox_id, || async move {
      let conn = check_pool.get().unwrap();
      assert_eq!(sandboxes::get(&conn, &check_id).unwrap().status, "stopping");
      Ok::<(), crate::sbx::Error>(())
    })
    .await;

    assert!(result.is_ok());
    let conn = pool.get().unwrap();
    assert_eq!(sandboxes::get(&conn, &sandbox_id).unwrap().status, "stopped");

    std::fs::remove_file(&path).ok();
  }
}

#[cfg(test)]
mod env_var_tests {
  use super::*;
  use r2d2_sqlite::SqliteConnectionManager;

  fn test_pool() -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("overnight-env-var-cmd-{}", uuid::Uuid::new_v4()));
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    crate::db::migrations::migrations().to_latest(&mut conn).unwrap();
    drop(conn);
    let pool = r2d2::Pool::builder().max_size(2).build(SqliteConnectionManager::file(&path)).unwrap();
    (pool, path)
  }

  fn ev(key: &str, value: &str) -> EnvVar {
    EnvVar { key: key.to_string(), value: value.to_string() }
  }

  #[test]
  fn validate_env_vars_rejects_bad_keys() {
    assert!(validate_env_vars(&[ev("GOOD_KEY", "1")]).is_ok());
    assert!(validate_env_vars(&[ev("_ok", "1")]).is_ok());
    assert!(validate_env_vars(&[ev("1BAD", "x")]).is_err());
    assert!(validate_env_vars(&[ev("bad key", "x")]).is_err());
    assert!(validate_env_vars(&[ev("", "x")]).is_err());
  }

  #[test]
  fn validate_env_vars_rejects_duplicates() {
    let err = validate_env_vars(&[ev("A", "1"), ev("A", "2")]).unwrap_err();
    assert!(err.contains("A"));
  }

  #[test]
  fn merged_env_vars_lets_higher_scope_win() {
    let (pool, path) = test_pool();
    let conn = pool.get().unwrap();
    let project = projects::create(&conn, "Overnight", "/repo", None, None).unwrap();
    settings::set_json(&conn, GLOBAL_ENV_VARS_KEY, &vec![ev("SHARED", "global"), ev("ONLY_GLOBAL", "g")]).unwrap();
    projects::set_env_vars(&conn, &project.id, &[ev("SHARED", "project"), ev("ONLY_PROJECT", "p")]).unwrap();
    drop(conn);

    let merged = merged_env_vars(&pool, &project.id, &[ev("SHARED", "sandbox")]).unwrap();
    let get = |k: &str| merged.iter().find(|v| v.key == k).map(|v| v.value.clone());
    assert_eq!(get("SHARED").as_deref(), Some("sandbox"));
    assert_eq!(get("ONLY_GLOBAL").as_deref(), Some("g"));
    assert_eq!(get("ONLY_PROJECT").as_deref(), Some("p"));

    std::fs::remove_file(&path).ok();
  }
}

#[cfg(test)]
mod secret_tests {
  use super::*;
  use r2d2_sqlite::SqliteConnectionManager;

  fn test_pool() -> (DbPool, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("overnight-secret-cmd-{}", uuid::Uuid::new_v4()));
    let mut conn = rusqlite::Connection::open(&path).unwrap();
    crate::db::migrations::migrations().to_latest(&mut conn).unwrap();
    drop(conn);
    let pool = r2d2::Pool::builder().max_size(2).build(SqliteConnectionManager::file(&path)).unwrap();
    (pool, path)
  }

  fn value_secret(env: &str, host: &str, value: &str) -> Secret {
    Secret {
      target: SecretTarget::Custom { env: env.to_string(), hosts: vec![host.to_string()], placeholder: None },
      source: SecretSource::Value { value: value.to_string() },
    }
  }

  fn service_secret(service: &str, value: &str) -> Secret {
    Secret {
      target: SecretTarget::Service { service: service.to_string() },
      source: SecretSource::Value { value: value.to_string() },
    }
  }

  #[test]
  fn validate_secrets_accepts_a_known_service() {
    assert!(validate_secrets(&[service_secret("anthropic", "sk-abc")]).is_ok());
  }

  #[test]
  fn validate_secrets_rejects_an_unknown_service() {
    let err = validate_secrets(&[service_secret("not-a-real-service", "x")]).unwrap_err();
    assert!(err.contains("not-a-real-service"));
  }

  #[test]
  fn validate_secrets_allows_an_empty_value_for_a_service_but_not_a_custom_secret() {
    assert!(validate_secrets(&[service_secret("anthropic", "")]).is_ok());
    assert!(validate_secrets(&[value_secret("API_KEY", "api.example.com", "")]).is_err());
  }

  #[test]
  fn validate_secrets_rejects_bad_custom_env_names() {
    assert!(validate_secrets(&[value_secret("API_KEY", "api.example.com", "1")]).is_ok());
    assert!(validate_secrets(&[value_secret("", "api.example.com", "x")]).is_err());
    assert!(validate_secrets(&[value_secret("1BAD", "api.example.com", "x")]).is_err());
  }

  #[test]
  fn validate_secrets_rejects_a_custom_secret_with_no_hosts() {
    let secret = Secret {
      target: SecretTarget::Custom { env: "API_KEY".to_string(), hosts: vec![], placeholder: None },
      source: SecretSource::Value { value: "x".to_string() },
    };
    assert!(validate_secrets(&[secret]).is_err());
  }

  #[test]
  fn validate_secrets_rejects_an_empty_reference_or_command() {
    let bad_ref = Secret {
      target: SecretTarget::Service { service: "anthropic".to_string() },
      source: SecretSource::Reference { reference: "".to_string(), refresh: None },
    };
    assert!(validate_secrets(&[bad_ref]).is_err());
    let bad_cmd = Secret {
      target: SecretTarget::Service { service: "anthropic".to_string() },
      source: SecretSource::Command { command: "".to_string(), refresh: None },
    };
    assert!(validate_secrets(&[bad_cmd]).is_err());
  }

  #[test]
  fn validate_secrets_rejects_duplicate_keys() {
    let err = validate_secrets(&[service_secret("anthropic", "1"), service_secret("anthropic", "2")]).unwrap_err();
    assert!(err.contains("anthropic"));
  }

  #[test]
  fn secrets_as_env_vars_exports_only_custom_secrets_with_a_placeholder() {
    let with_placeholder = Secret {
      target: SecretTarget::Custom { env: "API_KEY".to_string(), hosts: vec!["a.com".to_string()], placeholder: Some("sbx-cs-x".to_string()) },
      source: SecretSource::Value { value: "real".to_string() },
    };
    let without_placeholder = Secret {
      target: SecretTarget::Custom { env: "OTHER".to_string(), hosts: vec!["b.com".to_string()], placeholder: None },
      source: SecretSource::Value { value: "real2".to_string() },
    };
    let service = service_secret("github", "tok");

    let env_vars = secrets_as_env_vars(&[with_placeholder, without_placeholder, service]);
    assert_eq!(env_vars, vec![EnvVar { key: "API_KEY".to_string(), value: "sbx-cs-x".to_string() }]);
  }

  #[test]
  fn removed_secrets_finds_dropped_keys_only() {
    let previous = [value_secret("KEPT", "a.com", "1"), service_secret("github", "2")];
    let current = [value_secret("KEPT", "a.com", "1")];
    let removed = removed_secrets(&previous, &current);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].key(), "github");
  }

  #[test]
  fn merged_secrets_lets_higher_scope_win_by_key() {
    let (pool, path) = test_pool();
    let conn = pool.get().unwrap();
    let project = projects::create(&conn, "Overnight", "/repo", None, None).unwrap();
    settings::set_json(
      &conn,
      GLOBAL_SECRETS_KEY,
      &vec![value_secret("SHARED", "g.com", "global"), service_secret("github", "g")],
    )
    .unwrap();
    projects::set_secrets(&conn, &project.id, &[value_secret("SHARED", "p.com", "project")]).unwrap();
    drop(conn);

    let merged = merged_secrets(&pool, &project.id, &[value_secret("SHARED", "s.com", "sandbox")]).unwrap();
    let shared = merged.iter().find(|s| s.key() == "SHARED").unwrap();
    match &shared.source {
      SecretSource::Value { value } => assert_eq!(value, "sandbox"),
      _ => panic!("expected a Value source"),
    }
    assert!(merged.iter().any(|s| s.key() == "github"));

    std::fs::remove_file(&path).ok();
  }
}

#[tauri::command]
pub async fn start_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  let name = sandbox.sbx_name.clone().ok_or_else(|| "sandbox has no sbx sandbox to start".to_string())?;

  check_free_memory()?;
  crate::sbx::resume(&app, &name).map_err(|e| e.to_string())?;

  // A stopped sandbox can't be `exec`'d into, so env var edits made while
  // it was stopped never reached it — push the current merged list now
  // that it's running again, same as a fresh create does in
  // provision_sandbox. Best-effort end to end: `sbx resume` above already
  // succeeded, so neither computing the merge nor pushing it should be
  // able to block the sandbox from coming back up / the DB status update.
  match merged_secrets(&pool, &sandbox.project_id, &sandbox.secrets) {
    Ok(merged) => {
      if let Err(e) = register_secrets(&app, &merged, Some(&name)).await {
        log::warn!("start_sandbox: failed to re-register secrets for {name}: {e}");
      }
    }
    Err(e) => log::warn!("start_sandbox: failed to compute merged secrets for {name}: {e}"),
  }

  match full_env_vars_for_sandbox(&pool, &sandbox.project_id, &sandbox) {
    Ok(merged) => {
      if let Err(e) = crate::sbx::set_env_vars(&app, &name, &merged).await {
        log::warn!("start_sandbox: failed to push env vars to {name}: {e}");
      }
    }
    Err(e) => log::warn!("start_sandbox: failed to compute merged env vars for {name}: {e}"),
  }

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, &id, "running", None, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String, force: bool) -> std::result::Result<(), String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  if let Some(name) = &sandbox.sbx_name {
    let rm_result = crate::sbx::rm(&app, name).await;
    finalize_sandbox_rm(name, rm_result, force).map_err(|e| e.to_string())?;
    if let Ok(dest) = plans_dest_dir(&app, name) {
      let _ = std::fs::remove_dir_all(&dest);
    }
  }

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::delete(&conn, &id).map_err(|e| e.to_string())
}

/// Decides whether a `sbx rm` outcome should block `delete_sandbox`'s DB
/// row removal. `force: true` only swallows `sbx::Error::NotFound` — any
/// other error (a real transient failure, a policy error, etc.) still
/// propagates, so a Retry after a genuine failure can't accidentally
/// force-delete the DB row for a sandbox that's actually still there.
/// Split out from `delete_sandbox` so this decision is unit-testable
/// against a hand-built `sbx::Result` instead of a real `sbx` install.
fn finalize_sandbox_rm(name: &str, rm_result: crate::sbx::Result<()>, force: bool) -> crate::sbx::Result<()> {
  match rm_result {
    Ok(()) => Ok(()),
    Err(crate::sbx::Error::NotFound) if force => {
      log::warn!("delete_sandbox: sbx rm reported {name} not found; force-deleting DB row only");
      Ok(())
    }
    Err(e) => Err(e),
  }
}

#[cfg(test)]
mod finalize_sandbox_rm_tests {
  use super::*;

  #[test]
  fn passes_through_success() {
    assert!(finalize_sandbox_rm("foo", Ok(()), false).is_ok());
  }

  #[test]
  fn propagates_not_found_when_not_forced() {
    assert!(matches!(finalize_sandbox_rm("foo", Err(crate::sbx::Error::NotFound), false), Err(crate::sbx::Error::NotFound)));
  }

  #[test]
  fn swallows_not_found_when_forced() {
    assert!(finalize_sandbox_rm("foo", Err(crate::sbx::Error::NotFound), true).is_ok());
  }

  #[test]
  fn still_propagates_other_errors_when_forced() {
    let result = finalize_sandbox_rm("foo", Err(crate::sbx::Error::CommandFailed("boom".into())), true);
    assert!(matches!(result, Err(crate::sbx::Error::CommandFailed(_))));
  }
}

#[cfg(test)]
mod plans_source_path_tests {
  use super::*;

  #[test]
  fn resolves_per_agent_plans_dir() {
    assert_eq!(plans_source_path("claude"), "/home/agent/.claude/plans");
    assert_eq!(plans_source_path("codex"), "/home/agent/.codex/plans");
  }

  #[test]
  fn falls_back_to_claude_for_unknown_agent() {
    assert_eq!(plans_source_path("gpt4"), "/home/agent/.claude/plans");
  }
}

#[derive(Serialize)]
pub struct SandboxUsage {
  pub input_tokens: i64,
  pub output_tokens: i64,
}

/// Sums token usage across every agent session that ran inside this
/// sandbox (see `providers::claude_code::launch_autonomous_session`).
#[tauri::command]
pub fn get_sandbox_usage(pool: State<DbPool>, id: String) -> std::result::Result<SandboxUsage, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let (input_tokens, output_tokens) = metrics::total_tokens_for_sandbox(&conn, &id).map_err(|e| e.to_string())?;
  Ok(SandboxUsage { input_tokens, output_tokens })
}

#[tauri::command]
pub fn get_backup_interval_minutes(pool: State<DbPool>) -> std::result::Result<i64, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  Ok(
    settings::get(&conn, crate::backup::BACKUP_INTERVAL_KEY)
      .map_err(|e| e.to_string())?
      .and_then(|raw| raw.parse().ok())
      .unwrap_or(crate::backup::DEFAULT_BACKUP_INTERVAL_MINUTES),
  )
}

#[tauri::command]
pub fn save_backup_interval_minutes(pool: State<DbPool>, minutes: i64) -> std::result::Result<(), String> {
  if minutes < 1 {
    return Err("backup interval must be at least 1 minute".to_string());
  }
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, crate::backup::BACKUP_INTERVAL_KEY, &minutes.to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_backup_keep_count(pool: State<DbPool>) -> i64 {
  crate::backup::keep_count(pool.inner())
}

/// Saves how many backups each sandbox keeps, then prunes any extras now.
#[tauri::command]
pub async fn save_backup_keep_count(pool: State<'_, DbPool>, count: i64) -> std::result::Result<(), String> {
  use crate::backup::{MAX_BACKUP_KEEP_COUNT as MAX, MIN_BACKUP_KEEP_COUNT as MIN};
  if !(MIN..=MAX).contains(&count) {
    return Err(format!("backups kept must be between {MIN} and {MAX}"));
  }
  let pool = pool.inner().clone();
  tauri::async_runtime::spawn_blocking(move || {
    let conn = pool.get().map_err(|e| e.to_string())?;
    settings::set(&conn, crate::backup::BACKUP_KEEP_COUNT_KEY, &count.to_string()).map_err(|e| e.to_string())?;
    crate::backup::prune_all(&pool);
    Ok(())
  })
  .await
  .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn save_sandbox_backup_settings(
  pool: State<DbPool>,
  id: String,
  enabled: bool,
  interval_minutes: Option<i64>,
) -> std::result::Result<Sandbox, String> {
  if interval_minutes.is_some_and(|m| m < 1) {
    return Err("backup interval must be at least 1 minute".to_string());
  }
  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::set_backup_settings(&conn, &id, enabled, interval_minutes).map_err(|e| e.to_string())
}

/// Whether `run_scheduler`'s periodic ticking is active — manual backups
/// and the pre-stop/pre-delete consent backup always work regardless.
#[tauri::command]
pub fn get_auto_backup_enabled(pool: State<DbPool>) -> std::result::Result<bool, String> {
  Ok(crate::backup::is_auto_backup_enabled(pool.inner()))
}

#[tauri::command]
pub fn save_auto_backup_enabled(pool: State<DbPool>, enabled: bool) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  settings::set(&conn, crate::backup::AUTO_BACKUP_ENABLED_KEY, if enabled { "true" } else { "false" })
    .map_err(|e| e.to_string())
}

/// Sandboxes with a `.claude`/`.git` copy currently in flight — polled by
/// the frontend to show a progress indicator and disable Stop/Delete.
#[tauri::command]
pub fn list_active_backups(app: AppHandle) -> Vec<crate::backup::ActiveBackup> {
  crate::backup::active_backups(&app)
}

/// Manually triggers a backup outside the scheduler — used by the Backup
/// menu (scope chosen by the user) and by the stop/delete confirmation
/// dialog (always scope "all") when the user opts to back up first, instead
/// of the app doing it silently.
#[tauri::command]
pub async fn backup_sandbox_now(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
  trigger: String,
  scope: String,
) -> std::result::Result<SandboxBackup, String> {
  let scope = crate::backup::BackupScope::parse(&scope)?;
  let pool = pool.inner().clone();
  crate::backup::backup_sandbox(&app, &pool, &id, &trigger, scope).await
}

/// The frontend already holds the full sandbox list in its store, so it
/// resolves display names (grouping headers, etc.) client-side rather than
/// this command joining them in.
#[tauri::command]
pub async fn list_backups(pool: State<'_, DbPool>) -> std::result::Result<Vec<SandboxBackup>, String> {
  let pool = pool.inner().clone();
  tauri::async_runtime::spawn_blocking(move || {
    let conn = pool.get().map_err(|e| e.to_string())?;
    backups::list_all(&conn).map_err(|e| e.to_string())
  })
  .await
  .map_err(|e| e.to_string())?
}

/// One entry in the global "operation in progress" indicator — a backup or
/// a restore, whichever `kind` says. `source_sandbox_id`/`scope` are only
/// meaningful for a restore; `trigger` only for a backup.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveOperation {
  /// What to pass to `cancel_backup` (sandbox id) or `cancel_restore` (operation id).
  pub id: String,
  pub kind: String,
  pub sandbox_id: String,
  pub source_sandbox_id: Option<String>,
  pub scope: Option<String>,
  pub trigger: Option<String>,
  pub started_at: i64,
}

#[tauri::command]
pub fn list_active_operations(app: AppHandle) -> Vec<ActiveOperation> {
  let backups = crate::backup::active_backups(&app).into_iter().map(|b| ActiveOperation {
    id: b.sandbox_id.clone(),
    kind: "backup".to_string(),
    sandbox_id: b.sandbox_id,
    source_sandbox_id: None,
    scope: None,
    trigger: Some(b.trigger),
    started_at: b.started_at,
  });
  let restores = crate::restore::active_restores(&app).into_iter().map(|r| ActiveOperation {
    id: r.id,
    kind: "restore".to_string(),
    sandbox_id: r.target_sandbox_id,
    source_sandbox_id: Some(r.source_sandbox_id),
    scope: Some(r.scope),
    trigger: None,
    started_at: r.started_at,
  });
  backups.chain(restores).collect()
}

#[tauri::command]
pub fn cancel_backup(app: AppHandle, sandbox_id: String) -> bool {
  crate::backup::cancel_backup(&app, &sandbox_id)
}

#[tauri::command]
pub fn cancel_restore(app: AppHandle, id: String) -> bool {
  crate::restore::cancel_restore(&app, &id)
}

/// Deletes one backup: its host directory (best-effort — an orphaned
/// directory is harmless, so a removal failure doesn't block deleting the
/// row) and its `sandbox_backups` row.
#[tauri::command]
pub fn delete_sandbox_backup(pool: State<DbPool>, id: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let backup = backups::get(&conn, &id).map_err(|e| e.to_string())?;
  if let Err(e) = std::fs::remove_dir_all(&backup.host_dir) {
    log::warn!("delete_sandbox_backup: failed to remove {}: {e}", backup.host_dir);
  }
  backups::delete(&conn, &id).map_err(|e| e.to_string())
}

/// Deletes every backup belonging to `sandbox_id` — its host directories
/// (best-effort, same reasoning as `delete_sandbox_backup`) and their rows.
/// Works for a sandbox that no longer exists, since backups aren't scoped
/// to a live sandbox row.
#[tauri::command]
pub fn delete_sandbox_backups_for_sandbox(pool: State<DbPool>, sandbox_id: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let victims = backups::list_for_sandbox(&conn, &sandbox_id).map_err(|e| e.to_string())?;
  for victim in victims {
    if let Err(e) = std::fs::remove_dir_all(&victim.host_dir) {
      log::warn!("delete_sandbox_backups_for_sandbox: failed to remove {}: {e}", victim.host_dir);
    }
  }
  backups::delete_all_for_sandbox(&conn, &sandbox_id).map_err(|e| e.to_string())
}

/// Restores a backup's `.claude` and/or `.git` copy (per `scope`) back into
/// a running sandbox.
#[tauri::command]
pub async fn restore_backup(
  app: AppHandle,
  pool: State<'_, DbPool>,
  backup_id: String,
  target_sandbox_id: String,
  scope: String,
) -> std::result::Result<(), String> {
  let scope = crate::backup::BackupScope::parse(&scope)?;
  let pool = pool.inner().clone();
  crate::restore::restore_backup(&app, &pool, &backup_id, &target_sandbox_id, scope).await
}

/// This sandbox's agent's plans directory inside the sandbox — resynced to
/// the host each time the Plans tab loads or "Resync" is clicked (unlike
/// the agent-data backup above, this path is re-synced in place rather
/// than versioned per timestamp, since it's meant to always reflect the
/// current in-sandbox plan files). Falls back to Claude's own plans dir
/// for an unknown/legacy agent id rather than failing the sync outright.
fn plans_source_path(agent: &str) -> String {
  crate::agents::get(agent).map(crate::agents::plans_dir).unwrap_or_else(|| crate::agents::plans_dir(&crate::agents::CLAUDE))
}

#[derive(Serialize)]
pub struct PlanFile {
  pub name: String,
  pub content: String,
  pub modified_at: i64,
}

fn plans_dest_dir(app: &AppHandle, sbx_name: &str) -> std::result::Result<std::path::PathBuf, String> {
  Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("plans").join(sbx_name))
}

fn read_plan_files(dir: &std::path::Path) -> std::io::Result<Vec<PlanFile>> {
  let mut plans = Vec::new();
  if !dir.exists() {
    return Ok(plans);
  }
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
      continue;
    }
    let name = entry.file_name().to_string_lossy().to_string();
    let content = std::fs::read_to_string(&path)?;
    let modified_at = entry
      .metadata()?
      .modified()
      .ok()
      .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
      .map(|d| d.as_millis() as i64)
      .unwrap_or(0);
    plans.push(PlanFile { name, content, modified_at });
  }
  plans.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
  Ok(plans)
}

/// Wipes and re-copies this sandbox's agent's `plans` directory (see
/// `plans_source_path`) to `<app_data_dir>/plans/<sbx_name>/` (rather than
/// trusting `sbx cp`'s overwrite behavior, which is unverified — see
/// `crate::sbx::cp_from_sandbox`) so a plan deleted inside the sandbox
/// doesn't linger on the host, then returns every `.md` file found there.
#[tauri::command]
pub async fn sync_sandbox_plans(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Vec<PlanFile>, String> {
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  if sandbox.status != "running" {
    return Err("sandbox must be running to sync plans".to_string());
  }
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox yet".to_string())?;

  let dest = plans_dest_dir(&app, &name)?;
  if dest.exists() {
    std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
  }
  std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

  let source_path = plans_source_path(&sandbox.agent);
  crate::sbx::cp_from_sandbox(&app, &name, &source_path, &dest.to_string_lossy(), None)
    .await
    .map_err(|e| e.to_string())?;

  // `sbx cp`'s exact nesting behavior is unverified (see the comment above)
  // — it may copy `plans/`'s *contents* into `dest`, or nest `plans/`
  // itself one level inside it. Prefer the nested form if present so this
  // works either way, rather than guessing wrong and silently finding
  // nothing.
  let nested = dest.join("plans");
  let source_dir = if nested.is_dir() { nested } else { dest };
  read_plan_files(&source_dir).map_err(|e| e.to_string())
}

/// Reads this sandbox's current branch/branch-list/worktrees (one `sbx
/// exec` round trip via `sbx::read_branch_snapshot`) and persists them —
/// shared by `get_sandbox_branch_info`'s live reload and the best-effort
/// snapshot `stop_sandbox_with` takes right before tearing the sandbox
/// down, so both go through the same read-then-record path.
async fn capture_branch_snapshot(
  app: &AppHandle,
  pool: &DbPool,
  id: &str,
  name: &str,
) -> std::result::Result<Sandbox, String> {
  let workspace_path = crate::sbx::workspace_path(app, name).await.map_err(|e| e.to_string())?;
  let snapshot =
    crate::sbx::read_branch_snapshot(app, name, workspace_path.as_deref()).await.map_err(|e| e.to_string())?;
  let at = crate::db::models::now_millis();
  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::record_branch_snapshot(&conn, id, snapshot.current_branch.as_deref(), &snapshot.branches, &snapshot.worktrees, at)
    .map_err(|e| e.to_string())
}

/// Backs the Branch tab. While the sandbox is running, refreshes the
/// persisted branch snapshot before returning (so the tab always shows a
/// live read, not a possibly-stale one); otherwise just returns the
/// already-persisted row — a stopped sandbox has no live state to read,
/// but still shows its last-known snapshot from before it stopped (see
/// `stop_sandbox_with`'s best-effort snapshot).
#[tauri::command]
pub async fn get_sandbox_branch_info(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  if sandbox.status != "running" {
    return Ok(sandbox);
  }
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox yet".to_string())?;
  capture_branch_snapshot(&app, &pool, &id, &name).await
}

const HOST_METRICS_RETENTION_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// Samples current host-wide CPU/memory/disk/network usage, records it,
/// and prunes samples older than `HOST_METRICS_RETENTION_MS`. Called on
/// the same poll loop the frontend already uses for sandbox status, so
/// this runs roughly every 5s (see `HostMonitor`'s doc comment for why it
/// must be reused across calls rather than recreated here).
#[tauri::command]
pub fn get_host_stats(pool: State<DbPool>, monitor: State<Mutex<crate::sbx::HostMonitor>>) -> Result<HostMetric> {
  let stats = {
    let mut monitor = monitor.lock().unwrap();
    crate::sbx::sample_host_stats(&mut monitor)
  };

  let conn = pool.get()?;
  let metric = host_metrics::record(&conn, &stats)?;
  host_metrics::prune_older_than(&conn, crate::db::models::now_millis() - HOST_METRICS_RETENTION_MS)?;
  Ok(metric)
}

/// History for seeding the PC stats panel's charts on page load, before
/// live polling (`get_host_stats`) takes over appending new points.
#[tauri::command]
pub fn get_host_stats_history(pool: State<DbPool>, since_ms: i64) -> Result<Vec<HostMetric>> {
  let conn = pool.get()?;
  host_metrics::list_since(&conn, since_ms)
}

/// Backs the Metrics tab's live polling: samples this sandbox's
/// CPU/memory/network usage via one `sbx exec` round trip
/// (`sbx::sample_resource_usage`), persists it, and prunes rows older
/// than `HOST_METRICS_RETENTION_MS` (reused as-is — one global retention
/// window for both host-wide and per-sandbox metrics). Only callable
/// while `status == "running"`: a stopped sandbox has no `/proc` to read.
/// Deliberately has no app-wide poll of its own — the Metrics tab starts
/// and clears its own 5s interval on mount/unmount, so this only runs
/// while that sandbox's detail page is actually open (see
/// `SandboxMetricsTab.tsx`), unlike `get_host_stats`'s single shared
/// app-wide poll.
#[tauri::command]
pub async fn get_sandbox_resource_usage(
  app: AppHandle,
  pool: State<'_, DbPool>,
  monitor: State<'_, Mutex<crate::sbx::SandboxMonitor>>,
  id: String,
) -> std::result::Result<ContainerMetric, String> {
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  if sandbox.status != "running" {
    return Err("sandbox must be running to sample resource usage".to_string());
  }
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox yet".to_string())?;

  let usage = crate::sbx::sample_resource_usage(&app, &name, &id, monitor.inner())
    .await
    .map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  let metric = container_metrics::record_for_sandbox(
    &conn,
    &id,
    usage.cpu_percent,
    usage.memory_mb,
    usage.network_rx_kb_per_sec,
    usage.network_tx_kb_per_sec,
  )
  .map_err(|e| e.to_string())?;
  container_metrics::prune_older_than(&conn, crate::db::models::now_millis() - HOST_METRICS_RETENTION_MS)
    .map_err(|e| e.to_string())?;
  Ok(metric)
}

/// History for seeding the Metrics tab's charts on mount, before live
/// polling (`get_sandbox_resource_usage`) takes over appending new points
/// — same role as `get_host_stats_history`, scoped to one sandbox.
#[tauri::command]
pub fn get_sandbox_resource_history(pool: State<DbPool>, sandbox_id: String, since_ms: i64) -> Result<Vec<ContainerMetric>> {
  let conn = pool.get()?;
  container_metrics::list_for_sandbox_since(&conn, &sandbox_id, since_ms)
}

#[tauri::command]
pub fn is_ssh_setup(app: AppHandle) -> bool {
  crate::sbx::ssh_is_setup(&app)
}

#[tauri::command]
pub async fn setup_sandbox_ssh(app: AppHandle) -> std::result::Result<(), String> {
  crate::sbx::setup_ssh(&app).await.map_err(|e| e.to_string())?;
  crate::sbx::fix_ssh_config_permissions();
  Ok(())
}

/// Opens VS Code's Remote-SSH into the sandbox. Expects `<name>.sbx` to
/// resolve already — the frontend checks `is_ssh_setup` and runs
/// `setup_sandbox_ssh` first when needed. The folder to open is asked from `sbx`
/// itself (`sbx ls`'s WORKSPACE column) rather than read from our own DB
/// row — that's the same path a plain `sbx exec`/`sbx run` attach lands
/// you in by default, and it's correct for both mount mode (host repo
/// path) and clone mode (in-VM clone path) without us having to track or
/// translate it ourselves.
#[tauri::command]
pub async fn open_sandbox_vscode(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<(), String> {
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;
  let remote = format!("ssh-remote+{name}.sbx");

  crate::sbx::fix_ssh_config_permissions();
  crate::sbx::clear_stale_host_key(&name);
  let workspace_path = crate::sbx::workspace_path(&app, &name).await.map_err(|e| e.to_string())?;
  log::info!("open_sandbox_vscode: sbx ls reports workspace path {workspace_path:?}");

  // `code` is a .cmd shim on Windows. Naming it bare ("code") makes
  // CreateProcessW fail to find it at all ("program not found"); routing
  // through `cmd /C code ...` finds it but corrupts multi-flag argument
  // lines like `--folder-uri <uri>` somewhere in that double hop. Naming
  // the file explicitly ("code.cmd") lets CreateProcessW resolve and run
  // it directly in one hop, with Rust's normal argv handling intact.
  #[cfg(target_os = "windows")]
  let mut cmd = std::process::Command::new("code.cmd");
  #[cfg(not(target_os = "windows"))]
  let mut cmd = std::process::Command::new("code");

  // `--folder-uri` encodes the remote authority and path as one URI
  // string, unlike `--remote <host> <path>` which passes the path as a
  // separate bare positional arg — safer on Windows, where a leading `/`
  // in a plain CLI arg can be mis-parsed as a switch character.
  match workspace_path {
    Some(path) => {
      cmd.arg("--folder-uri").arg(format!("vscode-remote://{remote}{path}"));
    }
    None => {
      cmd.arg("--remote").arg(&remote);
    }
  }
  log::info!(
    "open_sandbox_vscode: spawning {:?} {:?}",
    cmd.get_program(),
    cmd.get_args().collect::<Vec<_>>()
  );
  let program = cmd.get_program().to_string_lossy().to_string();
  let cmd_args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
  crate::git::hide_console(&mut cmd);
  let result = cmd.spawn();
  log_spawn_result(&pool, "Open sandbox in VS Code", &program, &cmd_args, &result);
  match result {
    Ok(child) => {
      log::info!("open_sandbox_vscode: spawned pid {:?}", child.id());
      Ok(())
    }
    Err(e) => {
      log::error!("open_sandbox_vscode: spawn failed: {e}");
      Err(format!(
        "failed to launch VS Code ({e}) — make sure `code` is on your PATH (VS Code's \"Shell Command: Install 'code' command in PATH\")"
      ))
    }
  }
}

/// Logs the outcome of a fire-and-forget `std::process::Command::spawn()`
/// (VS Code, the file explorer, a terminal launcher) to the Developer >
/// Command Log page. `exit_code` is always `None` — nothing is awaited, so
/// there's no exit status, only whether the spawn itself succeeded.
fn log_spawn_result(
  pool: &State<DbPool>,
  operation: &str,
  program: &str,
  args: &[String],
  result: &std::io::Result<std::process::Child>,
) {
  let Ok(conn) = pool.get() else { return };
  let (success, stderr) = match result {
    Ok(_) => (true, None),
    Err(e) => (false, Some(e.to_string())),
  };
  let _ = command_log::append(&conn, operation, program, args, success, None, stderr.as_deref());
}

/// Opens the root folder every sandbox's backups are nested under
/// (`<app_data_dir>/sandbox-backups`), for the global Backups page's
/// "Open Folder" button — a level up from any single backup's `host_dir`.
/// Creates it first (best-effort) so this works even before the first
/// backup has run.
#[tauri::command]
pub fn open_backups_root_folder(app: AppHandle, pool: State<DbPool>) -> std::result::Result<(), String> {
  let root = app.path().app_data_dir().map_err(|e| e.to_string())?.join(crate::backup::BACKUPS_ROOT_DIR_NAME);
  if let Err(e) = std::fs::create_dir_all(&root) {
    log::warn!("open_backups_root_folder: failed to create {}: {e}", root.display());
  }
  open_path_in_explorer(pool, root.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_path_in_explorer(pool: State<DbPool>, path: String) -> std::result::Result<(), String> {
  #[cfg(target_os = "windows")]
  let (program, result) = ("explorer", std::process::Command::new("explorer").arg(&path).spawn());
  #[cfg(target_os = "macos")]
  let (program, result) = ("open", std::process::Command::new("open").arg(&path).spawn());
  #[cfg(target_os = "linux")]
  let (program, result) = ("xdg-open", std::process::Command::new("xdg-open").arg(&path).spawn());

  log_spawn_result(&pool, "Open path in file explorer", program, &[path], &result);
  result.map(|_| ()).map_err(|e| e.to_string())
}

/// `git fetch sandbox-<name>` on the host repo, fast-forwarding any local
/// branch that can take the update cleanly. Plain host-side git — no `sbx`
/// involved, since the daemon and remote are already wired up by `sbx`
/// itself. Never pushes and never force-updates.
#[tauri::command]
pub fn git_sync_sandbox(pool: State<DbPool>, id: String) -> std::result::Result<Vec<crate::git::BranchSyncOutcome>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;
  let project = projects::get(&conn, &sandbox.project_id).map_err(|e| e.to_string())?;

  let result = crate::git::sync_from_sandbox(&project.repo_path, &name).map_err(|e| e.to_string());
  let stderr = result.as_ref().err().cloned();
  let _ = command_log::append(
    &conn,
    "Sync sandbox branches",
    "git",
    &[project.repo_path.clone(), name],
    result.is_ok(),
    None,
    stderr.as_deref(),
  );
  if let Ok(outcomes) = &result {
    let _ = sandboxes::record_git_sync(&conn, &id, outcomes, crate::db::models::now_millis());
  }
  result
}

/// One branch's diff against `base_branch`: mount mode has exactly one
/// (the host's current branch, diffed against its live working tree);
/// clone mode has one per branch found on the sandbox's fetched
/// `sandbox-<name>` remote.
#[derive(Serialize)]
pub struct BranchDiff {
  pub branch: String,
  pub stat: String,
  pub patch: String,
}

/// One branch's diff *summary* against `base_branch` — no patch text, for
/// the Branches list view which only ever renders `stat`.
#[derive(Serialize)]
pub struct BranchStat {
  pub branch: String,
  pub stat: String,
}

#[derive(Serialize)]
pub struct SandboxDiffStats {
  pub base_branch: Option<String>,
  pub branches: Vec<BranchStat>,
}

/// Backs the Branches list, which only ever shows "N files changed +X -Y"
/// per branch. Skips computing/transferring full patch text, and computes
/// every branch's stat in parallel instead of one at a time.
#[tauri::command]
pub fn get_sandbox_diff_stats(pool: State<DbPool>, id: String) -> std::result::Result<SandboxDiffStats, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let Some(base_branch) = sandbox.base_branch.clone() else {
    return Ok(SandboxDiffStats { base_branch: None, branches: vec![] });
  };
  let project = projects::get(&conn, &sandbox.project_id).map_err(|e| e.to_string())?;

  let result: std::result::Result<SandboxDiffStats, String> = (|| {
    if sandbox.mode == "mount" {
      let branch = crate::git::current_branch(&project.repo_path).unwrap_or_else(|| "HEAD".to_string());
      let stat = crate::git::diff_stat(&project.repo_path, &base_branch, None).map_err(|e| e.to_string())?;
      return Ok(SandboxDiffStats { base_branch: Some(base_branch.clone()), branches: vec![BranchStat { branch, stat }] });
    }

    let name = sandbox.sbx_name.clone().ok_or_else(|| "sandbox isn't running".to_string())?;
    let remote = format!("sandbox-{name}");
    let sandbox_branches =
      crate::git::fetch_and_list_sandbox_branches(&project.repo_path, &name).map_err(|e| e.to_string())?;

    let branches = std::thread::scope(|scope| {
      let handles: Vec<_> = sandbox_branches
        .iter()
        .map(|branch| {
          let repo_path = project.repo_path.clone();
          let base_branch = base_branch.clone();
          let target_ref = format!("{remote}/{branch}");
          let branch = branch.clone();
          scope.spawn(move || -> std::result::Result<BranchStat, String> {
            let stat = crate::git::diff_stat(&repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())?;
            Ok(BranchStat { branch, stat })
          })
        })
        .collect::<Vec<_>>();
      handles.into_iter().map(|h| h.join().unwrap()).collect::<std::result::Result<Vec<_>, String>>()
    })?;

    Ok(SandboxDiffStats { base_branch: Some(base_branch.clone()), branches })
  })();

  let stderr = result.as_ref().err().cloned();
  let _ = command_log::append(
    &conn,
    "Diff sandbox stats against base branch",
    "git",
    &[project.repo_path.clone(), base_branch],
    result.is_ok(),
    None,
    stderr.as_deref(),
  );
  result
}

/// Commits `branch` added on top of `base_branch`. Mirrors this file's
/// mount/clone branching so the list can't go stale relative to what the
/// Branches page just fetched.
#[tauri::command]
pub fn get_branch_commits(
  pool: State<DbPool>,
  id: String,
  branch: String,
) -> std::result::Result<Vec<crate::git::CommitInfo>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let base_branch = sandbox.base_branch.clone().ok_or_else(|| "sandbox has no base branch".to_string())?;
  let project = projects::get(&conn, &sandbox.project_id).map_err(|e| e.to_string())?;

  let result: std::result::Result<Vec<crate::git::CommitInfo>, String> = (|| {
    if sandbox.mode == "mount" {
      return crate::git::log(&project.repo_path, &base_branch, None).map_err(|e| e.to_string());
    }

    let name = sandbox.sbx_name.clone().ok_or_else(|| "sandbox isn't running".to_string())?;
    crate::git::fetch_and_list_sandbox_branches(&project.repo_path, &name).map_err(|e| e.to_string())?;
    let target_ref = format!("sandbox-{name}/{branch}");
    crate::git::log(&project.repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())
  })();

  let stderr = result.as_ref().err().cloned();
  let _ = command_log::append(
    &conn,
    "List branch commits",
    "git",
    &[project.repo_path.clone(), base_branch, branch],
    result.is_ok(),
    None,
    stderr.as_deref(),
  );
  result
}

/// Backs the branch-detail (Diff) page, which only ever renders one
/// branch's patch. Mirrors `get_branch_commits`'s mount/clone branching.
#[tauri::command]
pub fn get_branch_diff(pool: State<DbPool>, id: String, branch: String) -> std::result::Result<BranchDiff, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let base_branch = sandbox.base_branch.clone().ok_or_else(|| "sandbox has no base branch".to_string())?;
  let project = projects::get(&conn, &sandbox.project_id).map_err(|e| e.to_string())?;

  let result: std::result::Result<BranchDiff, String> = (|| {
    if sandbox.mode == "mount" {
      let stat = crate::git::diff_stat(&project.repo_path, &base_branch, None).map_err(|e| e.to_string())?;
      let patch = crate::git::diff(&project.repo_path, &base_branch, None).map_err(|e| e.to_string())?;
      return Ok(BranchDiff { branch: branch.clone(), stat, patch });
    }

    let name = sandbox.sbx_name.clone().ok_or_else(|| "sandbox isn't running".to_string())?;
    crate::git::fetch_and_list_sandbox_branches(&project.repo_path, &name).map_err(|e| e.to_string())?;
    let target_ref = format!("sandbox-{name}/{branch}");
    let stat = crate::git::diff_stat(&project.repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())?;
    let patch = crate::git::diff(&project.repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())?;
    Ok(BranchDiff { branch: branch.clone(), stat, patch })
  })();

  let stderr = result.as_ref().err().cloned();
  let _ = command_log::append(
    &conn,
    "Diff one branch against base branch",
    "git",
    &[project.repo_path.clone(), base_branch, branch],
    result.is_ok(),
    None,
    stderr.as_deref(),
  );
  result
}

#[tauri::command]
pub fn open_sandbox_terminal(
  pool: State<DbPool>,
  id: String,
  terminal_host: Option<String>,
) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;

  // Only the Windows block below consults this — keep it used unconditionally
  // so non-Windows targets don't warn about an unused parameter.
  let _ = &terminal_host;

  #[cfg(target_os = "windows")]
  {
    // Unset means "use the app-wide default" — resolved per launch, not
    // snapshotted onto the sandbox (unlike permission_mode at create time),
    // since this only affects which host app opens right now.
    let terminal_host = match terminal_host.filter(|h| !h.trim().is_empty()) {
      Some(h) => {
        if !VALID_TERMINAL_HOSTS.contains(&h.as_str()) {
          return Err(format!("invalid terminal host: {h}"));
        }
        h
      }
      None => settings::get(&conn, SANDBOX_TERMINAL_HOST_KEY)
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_TERMINAL_HOST.to_string()),
    };

    // Prefer a Windows Terminal tab (if `wt.exe` is on PATH) over a
    // standalone console window; fall back to today's `cmd /C start ...`
    // behavior on any spawn error (no PATH pre-check — see plan notes).
    let wt_profile = if terminal_host == "powershell" { "PowerShell" } else { "Command Prompt" };
    let wt_args: Vec<String> =
      ["-w", "0", "new-tab", "-p", wt_profile, "--", "sbx", "exec", "-it", name.as_str(), "bash"].map(String::from).to_vec();
    let wt_result = std::process::Command::new("wt.exe").args(&wt_args).spawn();
    let wt_spawned = wt_result.is_ok();
    log_spawn_result(&pool, "Open sandbox terminal", "wt.exe", &wt_args, &wt_result);

    if !wt_spawned {
      let fallback_args: Vec<String> = if terminal_host == "powershell" {
        vec!["/C".to_string(), "start".to_string(), "powershell".to_string(), "-NoExit".to_string(), "-Command".to_string(), format!("sbx exec -it {name} bash")]
      } else {
        vec!["/C".to_string(), "start".to_string(), "cmd".to_string(), "/K".to_string(), format!("sbx exec -it {name} bash")]
      };
      let fallback_result = std::process::Command::new("cmd").args(&fallback_args).spawn();
      log_spawn_result(&pool, "Open sandbox terminal", "cmd", &fallback_args, &fallback_result);
      fallback_result.map_err(|e| e.to_string())?;
    }
  }
  #[cfg(target_os = "macos")]
  {
    let script = format!("tell application \"Terminal\" to do script \"sbx exec -it {name} bash\"");
    let result = std::process::Command::new("osascript").arg("-e").arg(&script).spawn();
    log_spawn_result(&pool, "Open sandbox terminal", "osascript", &["-e".to_string(), script], &result);
    result.map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "linux")]
  {
    let script = format!("sbx exec -it {name} bash");
    let result = std::process::Command::new("x-terminal-emulator").arg("-e").arg(&script).spawn();
    log_spawn_result(&pool, "Open sandbox terminal", "x-terminal-emulator", &["-e".to_string(), script], &result);
    result.map_err(|e| e.to_string())?;
  }

  Ok(())
}

#[tauri::command]
pub fn open_sandbox_agent(
  pool: State<DbPool>,
  id: String,
  agent: String,
) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;
  if agent_cli_token(&agent).is_none() {
    return Err(format!("invalid agent: {agent}"));
  }

  #[cfg(target_os = "windows")]
  {
    // Always PowerShell, unlike open_sandbox_terminal — this button has no
    // cmd/powershell preference of its own. No agent name on the command —
    // a sandbox is bound to the one agent it was created with, so `sbx run`
    // already knows which one to attach to.
    let wt_args: Vec<String> = ["-w", "0", "new-tab", "-p", "PowerShell", "--", "sbx", "run", "--name", name.as_str()]
      .map(String::from)
      .to_vec();
    let wt_result = std::process::Command::new("wt.exe").args(&wt_args).spawn();
    let wt_spawned = wt_result.is_ok();
    log_spawn_result(&pool, "Open sandbox agent", "wt.exe", &wt_args, &wt_result);

    if !wt_spawned {
      let fallback_args: Vec<String> = vec![
        "/C".to_string(),
        "start".to_string(),
        "powershell".to_string(),
        "-NoExit".to_string(),
        "-Command".to_string(),
        format!("sbx run --name {name}"),
      ];
      let fallback_result = std::process::Command::new("cmd").args(&fallback_args).spawn();
      log_spawn_result(&pool, "Open sandbox agent", "cmd", &fallback_args, &fallback_result);
      fallback_result.map_err(|e| e.to_string())?;
    }
  }
  #[cfg(not(target_os = "windows"))]
  {
    let _ = &name;
  }

  Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn list_command_log(
  pool: State<DbPool>,
  limit: i64,
  offset: i64,
  success_only: Option<bool>,
  since_ms: Option<i64>,
  until_ms: Option<i64>,
  operation: Option<String>,
) -> std::result::Result<Vec<CommandLogEntry>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let filter = command_log::CommandLogFilter { success_only, since_ms, until_ms, operation: operation.as_deref() };
  command_log::list(&conn, limit, offset, &filter).map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn count_command_log(
  pool: State<DbPool>,
  success_only: Option<bool>,
  since_ms: Option<i64>,
  until_ms: Option<i64>,
  operation: Option<String>,
) -> std::result::Result<i64, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let filter = command_log::CommandLogFilter { success_only, since_ms, until_ms, operation: operation.as_deref() };
  command_log::count(&conn, &filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_command_log_operations(pool: State<DbPool>) -> std::result::Result<Vec<String>, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  command_log::list_operations(&conn).map_err(|e| e.to_string())
}
