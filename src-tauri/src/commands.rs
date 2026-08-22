use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::db::error::{Error, Result};
use crate::db::models::{ContainerMetric, JiraIssue, Project, Sandbox, Session, Task};
use crate::db::{container_metrics, jira_issues, metrics, projects, sandboxes, sessions, settings, tasks, DbPool};
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
// A sandbox is a Docker container running a fixed dev-tools image, bound
// either directly to a project's repo_path ("mount" mode) or to a fresh
// clone of it ("clone" mode). Errors are stringified rather than using
// `db::error::Error`/`Result` since they can originate from `docker::Error`
// too, matching the existing pattern used by the Jira commands above.

const SANDBOX_MEMORY_MB: u32 = 2048;
const SANDBOX_IMAGE: &str = "mcr.microsoft.com/devcontainers/universal";
const SANDBOX_CONTAINER_PORT: u16 = 8080;

fn sandboxes_dir(app: &AppHandle) -> std::result::Result<PathBuf, String> {
  let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("sandboxes");
  std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  Ok(dir)
}

fn check_free_memory() -> std::result::Result<(), String> {
  let free_mb = crate::docker::host_free_memory_mb();
  if free_mb < SANDBOX_MEMORY_MB as f64 {
    return Err(format!(
      "not enough free memory to start a sandbox: {free_mb:.0}MB free, {SANDBOX_MEMORY_MB}MB required"
    ));
  }
  Ok(())
}

#[tauri::command]
pub async fn docker_health_check(app: AppHandle) -> std::result::Result<(), String> {
  crate::docker::info(&app).await.map_err(|e| e.to_string())
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
) -> std::result::Result<Sandbox, String> {
  if mode != "mount" && mode != "clone" {
    return Err(format!("invalid sandbox mode: {mode} (expected \"mount\" or \"clone\")"));
  }
  let pool = pool.inner().clone();

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
    let initial_folder = if mode == "mount" { Some(project.repo_path.as_str()) } else { None };
    sandboxes::create(&conn, &project_id, &mode, initial_folder).map_err(|e| e.to_string())?
  };

  let folder_path = if mode == "clone" {
    let target = sandboxes_dir(&app)?.join(&sandbox.id);
    crate::docker::clone_repo(&app, &project.repo_path, &target).await.map_err(|e| e.to_string())?;
    let path = target.to_string_lossy().to_string();
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::set_folder_path(&conn, &sandbox.id, &path).map_err(|e| e.to_string())?;
    path
  } else {
    project.repo_path.clone()
  };

  let host_port = crate::docker::find_free_port().map_err(|e| e.to_string())?;
  let opts = crate::docker::RunOptions {
    image: SANDBOX_IMAGE,
    name: &format!("overnight-sandbox-{}", sandbox.id),
    mount: (&folder_path, crate::docker::CONTAINER_WORKDIR),
    host_port,
    container_port: SANDBOX_CONTAINER_PORT,
    memory_mb: SANDBOX_MEMORY_MB,
  };
  let container_id = crate::docker::run_container(&app, &opts).await.map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, &sandbox.id, "running", Some(&container_id), Some(host_port as i64))
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_sandbox(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  let container_id = sandbox.container_id.ok_or_else(|| "sandbox has no container to stop".to_string())?;
  crate::docker::stop(&app, &container_id).await.map_err(|e| e.to_string())?;

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
  let container_id = sandbox.container_id.ok_or_else(|| "sandbox has no container to start".to_string())?;

  check_free_memory()?;
  crate::docker::start(&app, &container_id).await.map_err(|e| e.to_string())?;

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
  if let Some(container_id) = sandbox.container_id {
    crate::docker::rm(&app, &container_id).await.map_err(|e| e.to_string())?;
  }

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::delete(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_sandbox_metrics(
  app: AppHandle,
  pool: State<'_, DbPool>,
  id: String,
) -> std::result::Result<ContainerMetric, String> {
  let pool = pool.inner().clone();
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  let container_id = sandbox.container_id.ok_or_else(|| "sandbox has no running container".to_string())?;
  let stats = crate::docker::stats(&app, &container_id).await.map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  container_metrics::record_for_sandbox(
    &conn,
    &id,
    stats.cpu_percent,
    stats.memory_mb,
    stats.network_rx_bytes,
    stats.network_tx_bytes,
  )
  .map_err(|e| e.to_string())
}

#[derive(Clone, Serialize)]
struct SandboxLogLine {
  sandbox_id: String,
  line: String,
}

/// Starts a background task streaming `docker logs -f` for a sandbox's
/// container, emitting each line as a `sandbox-log` webview event. Returns
/// immediately once the stream is set up — the frontend's LogsPanel
/// subscribes to the event rather than polling this command.
#[tauri::command]
pub fn stream_sandbox_logs(app: AppHandle, pool: State<DbPool>, id: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let container_id = sandbox.container_id.ok_or_else(|| "sandbox has no running container".to_string())?;

  let spawned = crate::docker::logs_stream(&app, &container_id).map_err(|e| e.to_string())?;
  let bg_app = app.clone();
  let sandbox_id = id.clone();
  tauri::async_runtime::spawn(async move {
    use futures::StreamExt;
    let mut lines = spawned.stdout_lines;
    while let Some(line) = lines.next().await {
      crate::process::emit_to_webview(&bg_app, "sandbox-log", SandboxLogLine { sandbox_id: sandbox_id.clone(), line });
    }
  });
  Ok(())
}

#[derive(Serialize)]
pub struct SandboxUsage {
  pub input_tokens: i64,
  pub output_tokens: i64,
}

/// Sums token usage across every agent session that ran inside this
/// sandbox's container (see `providers::claude_code::launch_autonomous_session`).
#[tauri::command]
pub fn get_sandbox_usage(pool: State<DbPool>, id: String) -> std::result::Result<SandboxUsage, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let (input_tokens, output_tokens) = metrics::total_tokens_for_sandbox(&conn, &id).map_err(|e| e.to_string())?;
  Ok(SandboxUsage { input_tokens, output_tokens })
}

#[tauri::command]
pub fn open_sandbox_vscode(pool: State<DbPool>, id: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let folder = sandbox.folder_path.ok_or_else(|| "sandbox has no folder to open".to_string())?;
  std::process::Command::new("code").arg(&folder).spawn().map_err(|e| e.to_string())?;
  Ok(())
}

#[tauri::command]
pub fn open_sandbox_terminal(pool: State<DbPool>, id: String) -> std::result::Result<(), String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let container_id = sandbox.container_id.ok_or_else(|| "sandbox has no running container".to_string())?;

  #[cfg(target_os = "windows")]
  {
    std::process::Command::new("cmd")
      .args(["/C", "start", "cmd", "/K", &format!("docker exec -it {container_id} bash")])
      .spawn()
      .map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "macos")]
  {
    let script = format!("tell application \"Terminal\" to do script \"docker exec -it {container_id} bash\"");
    std::process::Command::new("osascript").arg("-e").arg(script).spawn().map_err(|e| e.to_string())?;
  }
  #[cfg(target_os = "linux")]
  {
    std::process::Command::new("x-terminal-emulator")
      .arg("-e")
      .arg(format!("docker exec -it {container_id} bash"))
      .spawn()
      .map_err(|e| e.to_string())?;
  }

  Ok(())
}
