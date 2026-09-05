use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::error::{Error, Result};
use crate::db::models::{HostMetric, JiraIssue, Project, Sandbox, Session, Task};
use crate::db::{host_metrics, jira_issues, metrics, projects, sandboxes, sessions, settings, tasks, DbPool};
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
  extra_clone_paths: Vec<String>,
) -> Result<Project> {
  validate_repo_path(&repo_path)?;
  let conn = pool.get()?;
  projects::create(
    &conn,
    &name,
    &repo_path,
    plans_path.as_deref(),
    dev_server_port,
    &extra_clone_paths,
  )
}

#[tauri::command]
pub fn update_project(
  pool: State<DbPool>,
  id: String,
  name: String,
  repo_path: String,
  plans_path: Option<String>,
  dev_server_port: Option<i64>,
  extra_clone_paths: Vec<String>,
) -> Result<Project> {
  validate_repo_path(&repo_path)?;
  let conn = pool.get()?;
  projects::update(
    &conn,
    &id,
    &name,
    &repo_path,
    plans_path.as_deref(),
    dev_server_port,
    &extra_clone_paths,
  )
}

#[tauri::command]
pub fn delete_project(pool: State<DbPool>, id: String) -> Result<()> {
  let conn = pool.get()?;
  projects::delete(&conn, &id)
}

const CLAUDE_PERMISSION_MODE_KEY: &str = "default_claude_permission_mode";
const DEFAULT_CLAUDE_PERMISSION_MODE: &str = "default";
const VALID_PERMISSION_MODES: [&str; 4] = ["plan", "default", "acceptEdits", "bypassPermissions"];

#[derive(Serialize)]
pub struct AppSettings {
  pub default_claude_permission_mode: String,
}

#[tauri::command]
pub fn get_settings(pool: State<DbPool>) -> Result<AppSettings> {
  let conn = pool.get()?;
  Ok(AppSettings {
    default_claude_permission_mode: settings::get(&conn, CLAUDE_PERMISSION_MODE_KEY)?
      .unwrap_or_else(|| DEFAULT_CLAUDE_PERMISSION_MODE.to_string()),
  })
}

#[tauri::command]
pub fn save_settings(pool: State<DbPool>, default_claude_permission_mode: String) -> Result<()> {
  if !VALID_PERMISSION_MODES.contains(&default_claude_permission_mode.as_str()) {
    return Err(Error::InvalidValue(format!(
      "invalid permission mode: {default_claude_permission_mode}"
    )));
  }
  let conn = pool.get()?;
  settings::set(&conn, CLAUDE_PERMISSION_MODE_KEY, &default_claude_permission_mode)
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

fn check_free_memory() -> std::result::Result<(), String> {
  let free_mb = crate::sbx::host_free_memory_mb();
  if free_mb < SANDBOX_MEMORY_MB as f64 {
    return Err(format!(
      "not enough free memory to start a sandbox: {free_mb:.0}MB free, {SANDBOX_MEMORY_MB}MB recommended"
    ));
  }
  Ok(())
}

fn sbx_name_for(sandbox_id: &str) -> String {
  format!("overnight-{sandbox_id}")
}

#[tauri::command]
pub async fn sbx_health_check(app: AppHandle) -> std::result::Result<(), String> {
  crate::sbx::health_check(&app).await.map_err(|e| e.to_string())
}

const VALID_NETWORK_POLICY_PRESETS: [&str; 3] = ["allow-all", "balanced", "deny-all"];

/// One-time, machine-wide setup answering sbx's interactive network-policy
/// prompt headlessly. The frontend calls this when `create_sandbox` fails
/// with the "network policy hasn't been initialized" error.
#[tauri::command]
pub async fn init_sbx_policy(app: AppHandle, preset: String) -> std::result::Result<(), String> {
  if !VALID_NETWORK_POLICY_PRESETS.contains(&preset.as_str()) {
    return Err(format!("invalid network policy preset: {preset}"));
  }
  crate::sbx::policy_init(&app, &preset).await.map_err(|e| e.to_string())
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

#[tauri::command]
pub async fn create_sandbox(
  app: AppHandle,
  pool: State<'_, DbPool>,
  project_id: String,
  mode: String,
  name: Option<String>,
  permission_mode: Option<String>,
) -> std::result::Result<Sandbox, String> {
  if mode != "mount" && mode != "clone" {
    return Err(format!("invalid sandbox mode: {mode} (expected \"mount\" or \"clone\")"));
  }
  let name = name.filter(|n| !n.trim().is_empty());
  let pool = pool.inner().clone();

  // Unset means "use whatever's configured as the app-wide default" —
  // resolved and snapshotted onto the sandbox now rather than looked up
  // again on every session launch, same as folder_path/sbx_name are fixed
  // at creation time.
  let permission_mode = match permission_mode.filter(|m| !m.trim().is_empty()) {
    Some(m) => {
      if !VALID_PERMISSION_MODES.contains(&m.as_str()) {
        return Err(format!("invalid permission mode: {m}"));
      }
      m
    }
    None => {
      let conn = pool.get().map_err(|e| e.to_string())?;
      settings::get(&conn, CLAUDE_PERMISSION_MODE_KEY)
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_CLAUDE_PERMISSION_MODE.to_string())
    }
  };

  let project = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    projects::get(&conn, &project_id).map_err(|e| e.to_string())?
  };

  if mode == "mount" {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let existing = sandboxes::list_for_project(&conn, &project_id).map_err(|e| e.to_string())?;
    if existing.iter().any(|s| s.mode == "mount" && matches!(s.status.as_str(), "starting" | "running")) {
      return Err("a mount-mode sandbox is already running for this project".to_string());
    }
  }

  check_free_memory()?;

  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    // Clone mode's clone lives inside the sandbox VM, not on the host — no
    // host-visible folder_path to record for it.
    let initial_folder = if mode == "mount" { Some(project.repo_path.as_str()) } else { None };
    sandboxes::create(&conn, &project_id, &mode, initial_folder, name.as_deref(), &permission_mode).map_err(|e| e.to_string())?
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
  let name = sbx_name_for(&sandbox.id);
  crate::sbx::create(app, &name, mode == "clone", &project.repo_path).await.map_err(|e| e.to_string())?;
  if mode == "clone" {
    copy_extra_clone_paths(app, &name, project).await?;
  }
  crate::sbx::set_claude_default_permission_mode(app, &name, &sandbox.permission_mode)
    .await
    .map_err(|e| e.to_string())?;
  sync_git_identity(app, &name).await;
  crate::sbx::publish_port(app, &name, SANDBOX_PORT).await.map_err(|e| e.to_string())?;
  let host_port = crate::sbx::host_port(app, &name, SANDBOX_PORT).await.map_err(|e| e.to_string())?;

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
  let output = std::process::Command::new("git").args(["config", "--global", key]).output().ok()?;
  if !output.status.success() {
    return None;
  }
  let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
  (!value.is_empty()).then_some(value)
}

/// Clone mode's `sbx create --clone` clones the git-tracked contents of
/// `project.repo_path` into an isolated copy inside the sandbox VM — files
/// git ignores (`.env`, local config, etc.) don't come along, which is
/// exactly what `project.extra_clone_paths`' glob patterns are for. Each
/// match is copied in afterward via `sbx cp`, preserving its path relative
/// to `repo_path`.
async fn copy_extra_clone_paths(app: &AppHandle, name: &str, project: &Project) -> std::result::Result<(), String> {
  if project.extra_clone_paths.is_empty() {
    return Ok(());
  }
  let workspace = crate::sbx::workspace_path(app, name)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "could not resolve the sandbox's in-VM workspace path".to_string())?;
  let repo_path = std::path::Path::new(&project.repo_path);
  let targets = resolve_extra_clone_targets(repo_path, &project.extra_clone_paths)?;

  for (host_path, relative_posix) in targets {
    let remote_path = format!("{workspace}/{relative_posix}");
    if let Some((parent, _)) = relative_posix.rsplit_once('/') {
      crate::sbx::mkdir(app, name, &format!("{workspace}/{parent}")).await.map_err(|e| e.to_string())?;
    }
    crate::sbx::cp(app, &host_path.to_string_lossy(), name, &remote_path).await.map_err(|e| e.to_string())?;
  }
  Ok(())
}

/// Expands `patterns` (glob patterns relative to `repo_path`) against the
/// host filesystem and pairs each match with its path relative to
/// `repo_path`, in POSIX form (`/`-separated — the sandbox VM is always
/// POSIX regardless of host OS). Split out from `copy_extra_clone_paths` as
/// the one part of that flow that's pure/host-only and so can be unit
/// tested without a real `sbx` install.
fn resolve_extra_clone_targets(
  repo_path: &std::path::Path,
  patterns: &[String],
) -> std::result::Result<Vec<(std::path::PathBuf, String)>, String> {
  let mut targets = Vec::new();
  for pattern in patterns {
    let full_pattern = repo_path.join(pattern);
    let matches = glob::glob(&full_pattern.to_string_lossy()).map_err(|e| format!("invalid glob pattern {pattern:?}: {e}"))?;
    for entry in matches {
      let host_path = entry.map_err(|e| e.to_string())?;
      // A pattern outside repo_path (picked via the folder/file browser)
      // has no meaningful path relative to the repo — drop it straight into
      // the clone root under its own name rather than recreating its whole
      // host directory hierarchy there.
      let relative_posix = match host_path.strip_prefix(repo_path) {
        Ok(relative) => relative
          .components()
          .map(|c| c.as_os_str().to_string_lossy().into_owned())
          .collect::<Vec<_>>()
          .join("/"),
        Err(_) => host_path
          .file_name()
          .map(|n| n.to_string_lossy().into_owned())
          .ok_or_else(|| format!("cannot determine a destination name for {host_path:?}"))?,
      };
      targets.push((host_path, relative_posix));
    }
  }
  Ok(targets)
}

#[cfg(test)]
mod extra_clone_paths_tests {
  use super::resolve_extra_clone_targets;
  use std::fs;

  #[test]
  fn resolves_glob_patterns_to_repo_relative_posix_paths() {
    let dir = std::env::temp_dir().join(format!("overnight-extra-clone-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(dir.join("config")).unwrap();
    fs::write(dir.join(".env"), "SECRET=1").unwrap();
    fs::write(dir.join("config").join("local.json"), "{}").unwrap();
    fs::write(dir.join("tracked.rs"), "// not matched").unwrap();

    let patterns = vec![".env".to_string(), "config/*.json".to_string()];
    let mut targets = resolve_extra_clone_targets(&dir, &patterns).unwrap();
    targets.sort_by(|a, b| a.1.cmp(&b.1));

    let relative: Vec<&str> = targets.iter().map(|(_, r)| r.as_str()).collect();
    assert_eq!(relative, vec![".env", "config/local.json"]);

    fs::remove_dir_all(&dir).unwrap();
  }

  #[test]
  fn path_outside_repo_lands_at_clone_root_under_its_own_name() {
    let repo_dir = std::env::temp_dir().join(format!("overnight-extra-clone-repo-{}", uuid::Uuid::new_v4()));
    let outside_dir = std::env::temp_dir().join(format!("overnight-extra-clone-outside-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&repo_dir).unwrap();
    fs::create_dir_all(outside_dir.join("cache")).unwrap();
    fs::write(outside_dir.join("cache").join("token"), "secret").unwrap();

    let patterns = vec![outside_dir.to_string_lossy().into_owned()];
    let targets = resolve_extra_clone_targets(&repo_dir, &patterns).unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].1, outside_dir.file_name().unwrap().to_string_lossy());

    fs::remove_dir_all(&repo_dir).unwrap();
    fs::remove_dir_all(&outside_dir).unwrap();
  }

  #[test]
  fn no_matches_yields_empty_targets() {
    let dir = std::env::temp_dir().join(format!("overnight-extra-clone-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let targets = resolve_extra_clone_targets(&dir, &["nonexistent/*.env".to_string()]).unwrap();
    assert!(targets.is_empty());

    fs::remove_dir_all(&dir).unwrap();
  }
}

#[tauri::command]
pub async fn stop_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox to stop".to_string())?;
  crate::sbx::stop(&app, &name).await.map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, &id, "stopped", None, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox to start".to_string())?;

  check_free_memory()?;
  crate::sbx::resume(&app, &name).map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, &id, "running", None, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<(), String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  if let Some(name) = sandbox.sbx_name {
    crate::sbx::rm(&app, &name).await.map_err(|e| e.to_string())?;
  }

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::delete(&conn, &id).map_err(|e| e.to_string())
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

const HOST_METRICS_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

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

/// Opens VS Code's Remote-SSH into the sandbox, running `sbx setup ssh`
/// first so the `<name>.sbx` SSH host is always registered (no manual
/// one-time setup required). The folder to open is asked from `sbx`
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

  // Regenerates the managed `Host *.sbx` SSH config block so `<name>.sbx`
  // resolves — documented as safe to re-run, so this replaces requiring
  // the user to run `sbx setup ssh` manually once beforehand.
  crate::sbx::setup_ssh(&app).await.map_err(|e| e.to_string())?;
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
  match cmd.spawn() {
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

#[tauri::command]
pub fn open_path_in_explorer(path: String) -> std::result::Result<(), String> {
  #[cfg(target_os = "windows")]
  {
    std::process::Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "macos")]
  {
    std::process::Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "linux")]
  {
    std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
  }

  Ok(())
}

#[tauri::command]
pub fn open_sandbox_terminal(pool: State<DbPool>, id: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;

  #[cfg(target_os = "windows")]
  {
    std::process::Command::new("cmd")
      .args(["/C", "start", "cmd", "/K", &format!("sbx exec -it {name} bash")])
      .spawn()
      .map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "macos")]
  {
    let script = format!("tell application \"Terminal\" to do script \"sbx exec -it {name} bash\"");
    std::process::Command::new("osascript").arg("-e").arg(script).spawn().map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "linux")]
  {
    std::process::Command::new("x-terminal-emulator")
      .arg("-e")
      .arg(format!("sbx exec -it {name} bash"))
      .spawn()
      .map_err(|e| e.to_string())?;
  }

  Ok(())
}
