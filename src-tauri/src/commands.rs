use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::error::{Error, Result};
use crate::db::models::{JiraIssue, Project, Sandbox, Session, Task};
use crate::db::{jira_issues, metrics, projects, sandboxes, sessions, settings, tasks, DbPool};
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
  crate::sbx::set_claude_default_permission_mode(app, &name, &sandbox.permission_mode)
    .await
    .map_err(|e| e.to_string())?;
  crate::sbx::publish_port(app, &name, SANDBOX_PORT).await.map_err(|e| e.to_string())?;
  let host_port = crate::sbx::host_port(app, &name, SANDBOX_PORT).await.map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::update_status(&conn, &sandbox.id, "running", Some(&name), host_port.map(i64::from))
    .map_err(|e| e.to_string())
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
