use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::db::error::{Error, Result};
use crate::db::models::{ContainerMetric, HostMetric, JiraIssue, Project, Sandbox, Session, Task};
use crate::db::{container_metrics, host_metrics, jira_issues, metrics, projects, sandboxes, sessions, settings, tasks, DbPool};
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

const CLAUDE_PERMISSION_MODE_KEY: &str = "default_claude_permission_mode";
const DEFAULT_CLAUDE_PERMISSION_MODE: &str = "default";
const VALID_PERMISSION_MODES: [&str; 4] = ["plan", "default", "acceptEdits", "bypassPermissions"];
const SKILL_FOLDERS_KEY: &str = "skill_folders";

#[derive(Serialize)]
pub struct AppSettings {
  pub default_claude_permission_mode: String,
  pub skill_folders: Vec<String>,
}

#[tauri::command]
pub fn get_settings(pool: State<DbPool>) -> Result<AppSettings> {
  let conn = pool.get()?;
  Ok(AppSettings {
    default_claude_permission_mode: settings::get(&conn, CLAUDE_PERMISSION_MODE_KEY)?
      .unwrap_or_else(|| DEFAULT_CLAUDE_PERMISSION_MODE.to_string()),
    skill_folders: settings::get_json(&conn, SKILL_FOLDERS_KEY)?.unwrap_or_default(),
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
/// `default_claude_permission_mode` — since it's a harmless no-op elsewhere.
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
    let created = match sandboxes::create(&conn, &project_id, mode, folder_path.as_deref(), None, DEFAULT_CLAUDE_PERMISSION_MODE, None) {
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
    assert_eq!(adopted[0].permission_mode, DEFAULT_CLAUDE_PERMISSION_MODE);
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
    let existing = sandboxes::create(&conn, &project.id, "mount", None, None, "default", None).unwrap();
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
    sandboxes::create(&conn, &project_id, &mode, initial_folder, name.as_deref(), &permission_mode, base_branch.as_deref())
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
  let base_name = base_sbx_name(sandbox.name.as_deref(), &project.name, &sandbox.id);
  let clone = mode == "clone";
  let name = resolve_unique_sbx_name(&base_name, |candidate| async move {
    crate::sbx::create(app, &candidate, clone, &project.repo_path).await.map_err(|e| e.to_string())
  })
  .await?;
  // `create` returning doesn't guarantee the VM is actually up for `exec`
  // yet — wait before the exec-dependent steps below.
  crate::sbx::wait_until_ready(app, &name, SANDBOX_READY_TIMEOUT, SANDBOX_READY_POLL_INTERVAL)
    .await
    .map_err(|e| e.to_string())?;

  // Independent once the sandbox is ready: git identity, the
  // permission-mode alias, and port publishing/lookup. Run concurrently.
  let (permission_result, host_port_result, ()) = tokio::join!(
    crate::sbx::set_claude_default_permission_mode(app, &name, &sandbox.permission_mode),
    async {
      crate::sbx::publish_port(app, &name, SANDBOX_PORT).await?;
      crate::sbx::host_port(app, &name, SANDBOX_PORT).await
    },
    sync_git_identity(app, &name),
  );
  permission_result.map_err(|e| e.to_string())?;
  let host_port = host_port_result.map_err(|e| e.to_string())?;

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
      sandboxes::create(&conn, &project.id, "mount", None, None, "default", None).unwrap().id
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
    if let Ok(dest) = plans_dest_dir(&app, &name) {
      let _ = std::fs::remove_dir_all(&dest);
    }
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

/// The `claude` CLI's own local config/state directory inside the sandbox —
/// the thing `backup_sandbox_claude_data` copies out.
const CLAUDE_DATA_SOURCE_PATH: &str = "/home/agent/.claude";

/// Copies this sandbox's in-VM `~/.claude` directory out to a host-side
/// backup folder under `<app_data_dir>/claude-backups/<sbx_name>/<unix_ms>/`
/// — deliberately outside any project's git repo, not user-configurable
/// this round. Only available while the sandbox is running (mirrors VS
/// Code/Terminal/Git Sync's gating); manual only, no automatic/scheduled
/// backups this round.
///
/// **UNVERIFIED**: no real `sbx` install is available in this dev
/// environment, so it's unconfirmed whether `sbx cp` creates a pre-existing
/// empty destination directory's *contents* from the source directory, or
/// nests the source directory itself one level inside it — this
/// pre-creates the destination (matching the plan's steps) rather than
/// guessing which.
#[tauri::command]
pub async fn backup_sandbox_claude_data(app: AppHandle, pool: State<'_, DbPool>, id: String) -> std::result::Result<Sandbox, String> {
  let sandbox = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    sandboxes::get(&conn, &id).map_err(|e| e.to_string())?
  };
  if sandbox.status != "running" {
    return Err("sandbox must be running to back up its Claude data".to_string());
  }
  let name = sandbox.sbx_name.ok_or_else(|| "sandbox has no sbx sandbox yet".to_string())?;

  let at = crate::db::models::now_millis();
  let dest = app
    .path()
    .app_data_dir()
    .map_err(|e| e.to_string())?
    .join("claude-backups")
    .join(&name)
    .join(at.to_string());
  std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
  let dest = dest.to_string_lossy().to_string();

  crate::sbx::cp_from_sandbox(&app, &name, CLAUDE_DATA_SOURCE_PATH, &dest)
    .await
    .map_err(|e| e.to_string())?;

  let conn = pool.get().map_err(|e| e.to_string())?;
  sandboxes::record_backup(&conn, &id, &dest, at).map_err(|e| e.to_string())
}

/// The `claude` CLI's plans directory inside the sandbox — resynced to the
/// host each time the Plans tab loads or "Resync" is clicked (unlike the
/// Claude-data backup above, this path is re-synced in place rather than
/// versioned per timestamp, since it's meant to always reflect the current
/// in-sandbox plan files).
const PLANS_SOURCE_PATH: &str = "/home/agent/.claude/plans";

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

/// Wipes and re-copies this sandbox's `/.claude/plans` to
/// `<app_data_dir>/plans/<sbx_name>/` (rather than trusting `sbx cp`'s
/// overwrite behavior, which is unverified — see `backup_sandbox_claude_data`)
/// so a plan deleted inside the sandbox doesn't linger on the host, then
/// returns every `.md` file found there.
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

  crate::sbx::cp_from_sandbox(&app, &name, PLANS_SOURCE_PATH, &dest.to_string_lossy())
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

  crate::git::sync_from_sandbox(&project.repo_path, &name).map_err(|e| e.to_string())
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

/// Backs the Diff tab. `base_branch: None` means this sandbox predates
/// branch tracking (or its host repo wasn't on a branch at creation time)
/// — there's no ref to diff against, so `branches` is always empty in that
/// case rather than guessing a fallback.
#[derive(Serialize)]
pub struct SandboxDiff {
  pub base_branch: Option<String>,
  pub branches: Vec<BranchDiff>,
}

/// Mount mode diffs `base_branch` against `project.repo_path`'s live
/// working tree directly — no fetch needed, since a mount-mode sandbox
/// shares the host's filesystem. Clone mode instead fetches
/// `sandbox-<name>` (`git::fetch_and_list_sandbox_branches`, the same
/// fetch-and-enumerate logic Git Sync uses) and three-dot diffs
/// `base_branch` against every branch found there.
#[tauri::command]
pub fn get_sandbox_diff(pool: State<DbPool>, id: String) -> std::result::Result<SandboxDiff, String> {
  let conn = pool.get().map_err(|e| e.to_string())?;
  let sandbox = sandboxes::get(&conn, &id).map_err(|e| e.to_string())?;
  let Some(base_branch) = sandbox.base_branch.clone() else {
    return Ok(SandboxDiff { base_branch: None, branches: vec![] });
  };
  let project = projects::get(&conn, &sandbox.project_id).map_err(|e| e.to_string())?;

  if sandbox.mode == "mount" {
    let branch = crate::git::current_branch(&project.repo_path).unwrap_or_else(|| "HEAD".to_string());
    let stat = crate::git::diff_stat(&project.repo_path, &base_branch, None).map_err(|e| e.to_string())?;
    let patch = crate::git::diff(&project.repo_path, &base_branch, None).map_err(|e| e.to_string())?;
    return Ok(SandboxDiff { base_branch: Some(base_branch), branches: vec![BranchDiff { branch, stat, patch }] });
  }

  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;
  let remote = format!("sandbox-{name}");
  let sandbox_branches =
    crate::git::fetch_and_list_sandbox_branches(&project.repo_path, &name).map_err(|e| e.to_string())?;

  let branches = sandbox_branches
    .into_iter()
    .map(|branch| {
      let target_ref = format!("{remote}/{branch}");
      let stat = crate::git::diff_stat(&project.repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())?;
      let patch = crate::git::diff(&project.repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())?;
      Ok(BranchDiff { branch, stat, patch })
    })
    .collect::<std::result::Result<Vec<_>, String>>()?;

  Ok(SandboxDiff { base_branch: Some(base_branch), branches })
}

/// Commits `branch` added on top of `base_branch`. Mirrors `get_sandbox_diff`'s
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

  if sandbox.mode == "mount" {
    return crate::git::log(&project.repo_path, &base_branch, None).map_err(|e| e.to_string());
  }

  let name = sandbox.sbx_name.ok_or_else(|| "sandbox isn't running".to_string())?;
  crate::git::fetch_and_list_sandbox_branches(&project.repo_path, &name).map_err(|e| e.to_string())?;
  let target_ref = format!("sandbox-{name}/{branch}");
  crate::git::log(&project.repo_path, &base_branch, Some(&target_ref)).map_err(|e| e.to_string())
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
    let wt_spawned = std::process::Command::new("wt.exe")
      .args(["-w", "0", "new-tab", "-p", wt_profile, "--", "sbx", "exec", "-it", &name, "bash"])
      .spawn()
      .is_ok();

    if !wt_spawned {
      if terminal_host == "powershell" {
        std::process::Command::new("cmd")
          .args(["/C", "start", "powershell", "-NoExit", "-Command", &format!("sbx exec -it {name} bash")])
          .spawn()
          .map_err(|e| e.to_string())?;
      } else {
        std::process::Command::new("cmd")
          .args(["/C", "start", "cmd", "/K", &format!("sbx exec -it {name} bash")])
          .spawn()
          .map_err(|e| e.to_string())?;
      }
    }
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
