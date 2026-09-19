//! Thin wrapper around the `sbx` CLI (Docker Sandboxes — microVM-isolated
//! agent sandboxes), built on `process::spawn`/`tauri_plugin_shell`. No
//! caller outside this module should shell out to `sbx` directly.
//!
//! Several things here are best-effort readings of https://docs.docker.com/ai/sandboxes/
//! rather than verified against a real `sbx` install (none is available in
//! this dev environment) — see the doc comment on each function that says so.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_shell::ShellExt;

use crate::db::models::{EnvVar, SecretSource, WorktreeInfo};
use crate::db::{command_log, DbPool};
use crate::process::SpawnedProcess;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("shell error: {0}")]
  Shell(#[from] tauri_plugin_shell::Error),
  #[error("process error: {0}")]
  Process(#[from] crate::process::Error),
  #[error("command failed: {0}")]
  CommandFailed(String),
  #[error("sbx's global network policy hasn't been initialized yet — run sbx policy init <allow-all|balanced|deny-all>")]
  PolicyNotInitialized,
  #[error("sbx's global network policy is already initialized — run sbx policy reset first to change it")]
  PolicyAlreadyInitialized,
  #[error("{0}")]
  ContainerStartFailed(String),
  #[error("sandbox not found — it may have already been removed outside the app")]
  NotFound,
  #[error("sbx {0:?} didn't finish within {1:?} — it may be waiting on an interactive prompt this app can't answer")]
  TimedOut(Vec<String>, std::time::Duration),
}

pub type Result<T> = std::result::Result<T, Error>;

async fn run<R: Runtime>(app: &AppHandle<R>, operation: &str, args: &[&str]) -> Result<String> {
  run_with_logged_args(app, operation, args, args).await
}

/// Like `run`, but persists `logged_args` to the command log instead of
/// the real `args` actually executed — used when the real args carry
/// sensitive values (e.g. env var values written into a sandbox's
/// persistent shell file) that shouldn't be readable later from the
/// Developer > Command Log page. `logged_args` also replaces `args` in
/// the error message on a non-zero exit, so a failure surfaced to the UI
/// doesn't leak the values either.
async fn run_with_logged_args<R: Runtime>(
  app: &AppHandle<R>,
  operation: &str,
  args: &[&str],
  logged_args: &[&str],
) -> Result<String> {
  let output = app.shell().command("sbx").args(args).output().await?;
  let success = output.status.success();
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
  record(
    app,
    operation,
    logged_args,
    success,
    output.status.code().map(i64::from),
    (!stderr.is_empty()).then_some(stderr.as_str()),
  );
  if !success {
    if let Some(err) = classify_command_failure(&stderr) {
      return Err(err);
    }
    return Err(Error::CommandFailed(format!(
      "sbx {logged_args:?} exited with {:?}: {stderr}",
      output.status.code(),
    )));
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// How long the two custom-secret calls below (`set_custom_secret`,
/// `remove_custom_secret`) are allowed to run before this app gives up on
/// them. `tauri_plugin_shell`'s `Command::output()` gives the child a
/// piped stdin whose write end it holds open for the whole call without
/// ever writing to or closing it — so if `sbx secret set-custom`/`rm
/// --host` (an experimental, undocumented surface — see the doc comments
/// below) ever falls back to an interactive confirmation prompt neither
/// flag we pass is confirmed to suppress, the child blocks reading stdin
/// forever and `output()` never returns to let us record or report it.
/// Every other call in this module keeps running with no bound, as it
/// always has — this is scoped to just these two because they're the
/// only ones exercising unverified command surface.
const SBX_SECRET_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Wraps `run_with_logged_args` with `SBX_SECRET_COMMAND_TIMEOUT`. On
/// timeout, records a failed command-log entry itself (the wrapped call
/// never got the chance to, since `output()` is still pending when the
/// timeout fires) so the Developer > Command Log page still shows that
/// something was attempted.
async fn run_with_logged_args_timed<R: Runtime>(
  app: &AppHandle<R>,
  operation: &str,
  args: &[&str],
  logged_args: &[&str],
) -> Result<String> {
  match tokio::time::timeout(SBX_SECRET_COMMAND_TIMEOUT, run_with_logged_args(app, operation, args, logged_args)).await {
    Ok(result) => result,
    Err(_) => {
      record(app, operation, logged_args, false, None, Some("timed out waiting for sbx"));
      Err(Error::TimedOut(logged_args.iter().map(|s| s.to_string()).collect(), SBX_SECRET_COMMAND_TIMEOUT))
    }
  }
}

/// Persists one row to the Developer > Command Log page. Best-effort: a
/// missing pool (not yet `app.manage`d, e.g. in a test) or a write failure
/// just skips logging rather than failing the underlying sbx call.
fn record<R: Runtime>(app: &AppHandle<R>, operation: &str, args: &[&str], success: bool, exit_code: Option<i64>, stderr: Option<&str>) {
  let Some(pool) = app.try_state::<DbPool>() else { return };
  let Ok(conn) = pool.get() else { return };
  let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
  if let Err(e) = command_log::append(&conn, operation, "sbx", &args, success, exit_code, stderr) {
    log::error!("failed to persist command log: {e}");
  }
}

/// Classifies a non-zero `sbx` exit's stderr into a specific `Error`
/// variant when it matches a known failure shape, so callers get something
/// more actionable than the generic `CommandFailed`. `None` when nothing
/// recognized matches. Pure/stderr-only (no `AppHandle`) so each pattern is
/// directly unit-testable without a real `sbx` install.
fn classify_command_failure(stderr: &str) -> Option<Error> {
  if stderr.contains("network policy has not been initialized") {
    return Some(Error::PolicyNotInitialized);
  }
  if stderr.contains("already initialized") {
    return Some(Error::PolicyAlreadyInitialized);
  }
  if stderr.contains("failed to run sandbox container") {
    return Some(Error::ContainerStartFailed(container_start_failed_message(stderr)));
  }
  // UNVERIFIED, same caveat as container_start_failed_message above: no
  // real `sbx` install is available in this dev environment to confirm
  // `sbx rm`'s exact "sandbox doesn't exist" stderr wording. Matches
  // tolerantly on two plausible phrasings — a too-narrow match just
  // regresses to CommandFailed (delete_sandbox still fails safely, the
  // force-delete dialog simply won't be offered), which is safer than a
  // too-broad match that could misclassify a real transient rm failure as
  // NotFound. Needs verifying against a real `sbx rm <nonexistent-name>`
  // failure before shipping.
  if stderr.contains("not found") || stderr.contains("no such sandbox") {
    return Some(Error::NotFound);
  }
  None
}

/// sbx runs agents in its own microVMs rather than plain Docker containers,
/// so this specific failure (image pull succeeds, but the VM itself won't
/// start) almost always means the host's hardware virtualization isn't
/// available to sbx — confirmed against a real repro where the cause was
/// Windows Hypervisor Platform being disabled (a hard prerequisite per
/// sbx's own install docs) despite Docker Desktop itself running fine.
/// Bakes in the exact check/fix commands per OS so the UI can surface them
/// directly instead of just the raw sbx error.
fn container_start_failed_message(stderr: &str) -> String {
  #[cfg(target_os = "windows")]
  let platform_hint = "This usually means Windows Hypervisor Platform is disabled — sbx requires it \
    even if Docker Desktop itself is running fine. Check it in an elevated (Run as administrator) \
    PowerShell:\n\n\
    Get-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform\n\n\
    If State shows Disabled, enable it in the same elevated window and reboot:\n\n\
    Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All";
  #[cfg(target_os = "macos")]
  let platform_hint = "This usually means sbx can't get hardware virtualization on this Mac — sbx \
    requires Apple silicon and macOS Sonoma (14) or later. Confirm both, then retry.";
  #[cfg(target_os = "linux")]
  let platform_hint = "This usually means KVM isn't available to sbx. Check with:\n\n\
    lsmod | grep kvm\n\n\
    If that's empty, run `kvm-ok` for diagnostics, and confirm your user is in the kvm group:\n\n\
    sudo usermod -aG kvm $USER";
  #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
  let platform_hint = "This usually means hardware virtualization isn't available to sbx on this host.";

  format!(
    "sbx failed to start the sandbox's virtual machine (the image pulled fine, but the container \
     itself wouldn't start). {platform_hint}\n\nRaw sbx output: {stderr}"
  )
}

/// `sbx setup ssh` — (re)generates the managed `Host *.sbx` block in the
/// user's SSH config so `<name>.sbx` resolves for `ssh`/VS Code Remote-SSH.
/// Documented as safe to re-run to regenerate the block, so callers can
/// invoke this before every VS Code launch instead of requiring the user
/// to run it once manually first.
pub async fn setup_ssh<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
  run(app, "Setup SSH", &["setup", "ssh"]).await?;
  Ok(())
}

/// Clears any stale `known_hosts` entry for `<name>.sbx` so a sandbox
/// recreated under the same name (new host key) doesn't get blocked by
/// OpenSSH's "REMOTE HOST IDENTIFICATION HAS CHANGED" refusal. Safe to
/// call even if no entry exists.
pub fn clear_stale_host_key(name: &str) {
  let host = format!("{name}.sbx");
  if let Err(e) = std::process::Command::new("ssh-keygen").args(["-R", &host]).output() {
    log::warn!("clear_stale_host_key: ssh-keygen -R {host} failed: {e}");
  }
}

/// Win32-OpenSSH's strict permission check refuses a config/known_hosts
/// file whose ACL grants access to anyone besides the owner/Administrators/
/// SYSTEM ("Bad owner or permissions..."), which can happen to sbx's
/// managed ssh directory (e.g. a stray inherited ACE) and blocks ssh.exe
/// before it even gets to host-key checks. Resets that directory's ACL to
/// just the current user. No-op on non-Windows or if the directory doesn't
/// exist yet.
#[cfg(target_os = "windows")]
pub fn fix_ssh_config_permissions() {
  let Ok(local_app_data) = std::env::var("LOCALAPPDATA") else { return };
  let ssh_dir = std::path::Path::new(&local_app_data).join("DockerSandboxes").join("sandboxes").join("config").join("ssh");
  if !ssh_dir.exists() {
    return;
  }
  let Ok(user) = std::env::var("USERNAME") else { return };
  let dir = ssh_dir.to_string_lossy().to_string();
  let run = |args: &[&str]| {
    if let Err(e) = std::process::Command::new("icacls").args(args).output() {
      log::warn!("fix_ssh_config_permissions: icacls {args:?} failed: {e}");
    }
  };
  run(&[&dir, "/inheritance:r", "/T", "/C"]);
  run(&[&dir, "/grant:r", &format!("{user}:(OI)(CI)F"), "/T", "/C"]);
}

#[cfg(not(target_os = "windows"))]
pub fn fix_ssh_config_permissions() {}

/// `sbx policy init <preset>` — one-time, machine-wide setup that answers
/// the interactive network-policy prompt headlessly. `preset` must be one
/// of `allow-all`, `balanced`, or `deny-all` (sbx's own accepted values).
pub async fn policy_init<R: Runtime>(app: &AppHandle<R>, preset: &str) -> Result<()> {
  run(app, "Initialize network policy", &["policy", "init", preset]).await?;
  Ok(())
}

/// `sbx policy reset --force` — clears the machine-wide policy (and every
/// custom rule) without the interactive re-prompt `sbx policy reset` shows
/// on its own. Confirmed against a real `sbx` install: `policy init` only
/// works the first time — changing an already-initialized preset requires
/// this first. Per sbx's own docs this restarts the network daemon and
/// **stops every currently running sandbox on the machine** — callers must
/// get explicit user confirmation before calling this, never invoke it
/// silently as part of an "apply preset" flow.
pub async fn policy_reset<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
  run(app, "Reset network policy", &["policy", "reset", "--force"]).await?;
  Ok(())
}

/// One row of `sbx policy ls [<sandbox-name>] --wide` output — a single
/// network rule, global or sandbox-scoped depending on what `policy_list`
/// was called with. `source` is sbx's own governance provenance (e.g.
/// `local`/`org`/`kit` per the docs) — callers use it to tell a
/// locally-added rule (removable) from an org-pushed one (read-only).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PolicyRule {
  pub host: String,
  pub decision: String,
  pub source: String,
}

/// `sbx policy allow network [--sandbox <name>] <host>[,<host2>,...]` —
/// adds one or more allow rules, either machine-wide (`sandbox: None`) or
/// scoped to one sandbox. `hosts` is passed through verbatim (sbx accepts
/// a comma-separated list per its own docs) — no local validation beyond
/// whatever sbx itself enforces.
pub async fn policy_allow<R: Runtime>(app: &AppHandle<R>, sandbox: Option<&str>, hosts: &str) -> Result<()> {
  policy_mutate(app, "allow", sandbox, hosts).await
}

/// `sbx policy deny network [--sandbox <name>] <host>[,<host2>,...]` —
/// same shape as `policy_allow`, opposite decision.
pub async fn policy_deny<R: Runtime>(app: &AppHandle<R>, sandbox: Option<&str>, hosts: &str) -> Result<()> {
  policy_mutate(app, "deny", sandbox, hosts).await
}

async fn policy_mutate<R: Runtime>(app: &AppHandle<R>, verb: &str, sandbox: Option<&str>, hosts: &str) -> Result<()> {
  let mut args = vec!["policy", verb, "network"];
  if let Some(name) = sandbox {
    args.push("--sandbox");
    args.push(name);
  }
  args.push(hosts);
  let operation = if verb == "allow" { "Allow network rule" } else { "Deny network rule" };
  run(app, operation, &args).await?;
  Ok(())
}

/// `sbx policy rm network [--sandbox <name>] --resource <host>` — removes
/// a rule by its resource (host pattern), not by sbx's internal rule id —
/// every caller here already knows the host it added/is removing, never
/// an opaque rule id.
pub async fn policy_rm<R: Runtime>(app: &AppHandle<R>, sandbox: Option<&str>, resource: &str) -> Result<()> {
  let mut args = vec!["policy", "rm", "network"];
  if let Some(name) = sandbox {
    args.push("--sandbox");
    args.push(name);
  }
  args.push("--resource");
  args.push(resource);
  run(app, "Remove network rule", &args).await?;
  Ok(())
}

/// `sbx policy ls [<sandbox-name>] --wide` — lists every active network
/// rule (machine-wide when `sandbox` is `None`, scoped when `Some`). This
/// is the *only* source of truth callers use for allow/deny lists — we
/// deliberately keep no local DB mirror of rule state, to avoid drift
/// against sbx's real rule store.
///
/// **UNVERIFIED**: no real `sbx` install is available in this dev
/// environment (see this module's top doc comment), so the exact `--wide`
/// column layout is a best-effort reading of the docs (a RESOURCE/HOST
/// column, a DECISION column, a SOURCE column) rather than something
/// confirmed against real output. `parse_policy_rules` is header-driven
/// (matches column *names*, not fixed positions) specifically so a
/// slightly different real layout degrades to an empty list instead of
/// misparsing silently.
pub async fn policy_list<R: Runtime>(app: &AppHandle<R>, sandbox: Option<&str>) -> Result<Vec<PolicyRule>> {
  let mut args = vec!["policy", "ls"];
  if let Some(name) = sandbox {
    args.push(name);
  }
  args.push("--wide");
  let output = run(app, "List network policy rules", &args).await?;
  Ok(parse_policy_rules(&output))
}

/// Confirmed against real `sbx policy ls <name> --wide` output (Windows,
/// v0.35+ era): the header uses a two-word `APPLIES TO` column, and a rule
/// with many resources wraps the extra ones onto continuation lines with
/// every earlier column left blank, e.g.:
///
/// ```text
/// SOURCE   APPLIES TO   POLICY   POLICY_ID      RULE   RULE_ID              TYPE      DECISION   RESOURCES
/// local    all          -        local-policy   -      default-ai-services  network   allow      api.anthropic.com:443
///                                                                                                 chatgpt.com:443
/// local    all          -        local-policy   -      default-fs-read...   filesystem:read  allow  **
/// ```
///
/// Rather than splitting on whitespace (which breaks on the two-word
/// header and can't tell a real row from a wrapped continuation), this
/// slices each line at the character offsets of the header's own column
/// names — offsets that sbx pads to fit that invocation's widest value, so
/// they're computed fresh per call rather than hardcoded. A continuation
/// line is identified by having nothing but whitespace before the
/// RESOURCES column; it's folded into the most recent real row's decision
/// (each resource becomes its own `PolicyRule`, since this app removes
/// rules by host, not by the named rule that groups many hosts together).
/// Only `TYPE == "network"` rows are kept — sbx also lists filesystem
/// rules here, which this app's network-policy UI has no use for.
fn parse_policy_rules(output: &str) -> Vec<PolicyRule> {
  let mut lines = output.lines();
  let header = match lines.find(|line| {
    let upper = line.to_uppercase();
    upper.contains("DECISION") && upper.contains("RESOURCES")
  }) {
    Some(header) => header,
    None => return Vec::new(),
  };

  let (Some(decision_start), Some(resources_start)) = (header.find("DECISION"), header.find("RESOURCES")) else {
    return Vec::new();
  };
  let type_start = header.find("TYPE");
  let source_end = header.find("APPLIES");

  fn slice(line: &str, start: usize, end: usize) -> String {
    let start = start.min(line.len());
    let end = end.min(line.len()).max(start);
    line.get(start..end).unwrap_or("").trim().to_string()
  }

  let mut rules = Vec::new();
  let mut current: Option<(String, String)> = None; // (decision, source) of the last kept network row

  for line in lines {
    if line.trim().is_empty() {
      continue;
    }
    let resource = slice(line, resources_start, line.len());
    if resource.is_empty() {
      continue;
    }
    let before_resources = slice(line, 0, resources_start);

    if before_resources.is_empty() {
      // Continuation line: another resource under the last row's rule.
      if let Some((decision, source)) = &current {
        rules.push(PolicyRule { host: resource, decision: decision.clone(), source: source.clone() });
      }
      continue;
    }

    let ty = type_start.map(|s| slice(line, s, decision_start)).unwrap_or_default();
    if !ty.eq_ignore_ascii_case("network") {
      current = None; // don't let this row's continuation lines get misread as the prior network row's
      continue;
    }

    let decision = slice(line, decision_start, resources_start);
    let source = source_end.map(|e| slice(line, 0, e)).filter(|s| !s.is_empty()).unwrap_or_else(|| "local".to_string());
    current = Some((decision.clone(), source.clone()));
    rules.push(PolicyRule { host: resource, decision, source });
  }

  rules
}

/// `sbx secret set anthropic -t <token>` — stores the Anthropic API key
/// globally (OS keychain), so every sandbox's Claude Code session
/// authenticates via the host-side proxy instead of needing an interactive
/// `/login` inside each sandbox.
pub async fn set_anthropic_secret<R: Runtime>(app: &AppHandle<R>, token: &str) -> Result<()> {
  run(app, "Set Anthropic API key", &["secret", "set", "anthropic", "-t", token]).await?;
  Ok(())
}

/// Appends the flags for `source` common to both `sbx secret set` and
/// `set-custom` — split out so both argument builders share one place
/// that knows the three source shapes. `--show-error` is only valid
/// alongside `--ref`/`--command` (`sbx` rejects it otherwise: "ERROR:
/// --show-error requires --ref or --command") so it's added here, next
/// to the two source kinds that resolve on the host and can fail to —
/// never for a literal `--value`, which has nothing to resolve.
/// `--no-verify` is never passed, so a bad source fails the registration
/// immediately instead of being stored unresolved.
fn push_source_args(args: &mut Vec<String>, source: &SecretSource) {
  match source {
    SecretSource::Value { value } => {
      args.push("--value".to_string());
      args.push(value.clone());
    }
    SecretSource::Reference { reference, refresh } => {
      args.push("--show-error".to_string());
      args.push("--ref".to_string());
      args.push(reference.clone());
      if let Some(r) = refresh {
        args.push("--refresh".to_string());
        args.push(r.clone());
      }
    }
    SecretSource::Command { command, refresh } => {
      args.push("--show-error".to_string());
      args.push("--command".to_string());
      args.push(command.clone());
      if let Some(r) = refresh {
        args.push("--refresh".to_string());
        args.push(r.clone());
      }
    }
  }
}

/// Replaces the value following `--value` with a redaction placeholder,
/// leaving everything else (including a `--ref`/`--command` source,
/// which is a pointer to a secret rather than the secret itself) intact.
fn redact_value_arg(args: &[String]) -> Vec<String> {
  let mut out = args.to_vec();
  if let Some(i) = out.iter().position(|a| a == "--value") {
    if let Some(v) = out.get_mut(i + 1) {
      *v = "********".to_string();
    }
  }
  out
}

/// Pure argument builder for `sbx secret set <service> [--sandbox
/// <name>] <source-flags>` — see `push_source_args` for the
/// `--show-error`/`--no-verify` handling.
fn service_secret_set_args(service: &str, source: &SecretSource, sandbox_name: Option<&str>) -> Vec<String> {
  let mut args = vec!["secret".to_string(), "set".to_string(), service.to_string()];
  if let Some(sbx) = sandbox_name {
    args.push("--sandbox".to_string());
    args.push(sbx.to_string());
  }
  push_source_args(&mut args, source);
  args
}

/// Stores one service secret (`anthropic`, `github`, ...). See
/// commands.rs::merged_secrets for scoping; a service secret contributes
/// nothing to a sandbox's own environment (see
/// commands.rs::secrets_as_env_vars) — it's consumed by the sandbox's
/// own agent/kit bootstrap.
pub async fn set_service_secret<R: Runtime>(
  app: &AppHandle<R>,
  service: &str,
  source: &SecretSource,
  sandbox_name: Option<&str>,
) -> Result<()> {
  let args = service_secret_set_args(service, source, sandbox_name);
  let logged = redact_value_arg(&args);
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let logged: Vec<&str> = logged.iter().map(String::as_str).collect();
  run_with_logged_args_timed(app, "Set service secret", &args, &logged).await?;
  Ok(())
}

/// Pure argument builder for `sbx secret set-custom [--sandbox <name>]
/// --host <h> [--host <h>...] --env <VAR> --placeholder <p>
/// <source-flags>` — see `push_source_args` for the
/// `--show-error`/`--no-verify` handling. `placeholder` is always
/// supplied explicitly by this app (see
/// commands.rs::resolve_secret_placeholders) — never omitted to let
/// `sbx` generate its own `{rand}` default — so re-registering the same
/// secret never changes the placeholder already exported into a
/// sandbox's environment.
fn custom_secret_set_args(env: &str, hosts: &[String], placeholder: &str, source: &SecretSource, sandbox_name: Option<&str>) -> Vec<String> {
  let mut args = vec!["secret".to_string(), "set-custom".to_string()];
  if let Some(sbx) = sandbox_name {
    args.push("--sandbox".to_string());
    args.push(sbx.to_string());
  }
  for host in hosts {
    args.push("--host".to_string());
    args.push(host.clone());
  }
  args.push("--env".to_string());
  args.push(env.to_string());
  args.push("--placeholder".to_string());
  args.push(placeholder.to_string());
  push_source_args(&mut args, source);
  args
}

/// Stores one custom secret. See commands.rs::merged_secrets for
/// scoping and commands.rs::secrets_as_env_vars for how `placeholder`
/// ends up as a real environment variable inside a sandbox.
pub async fn set_custom_secret<R: Runtime>(
  app: &AppHandle<R>,
  env: &str,
  hosts: &[String],
  placeholder: &str,
  source: &SecretSource,
  sandbox_name: Option<&str>,
) -> Result<()> {
  let args = custom_secret_set_args(env, hosts, placeholder, source, sandbox_name);
  let logged = redact_value_arg(&args);
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let logged: Vec<&str> = logged.iter().map(String::as_str).collect();
  run_with_logged_args_timed(app, "Set custom secret", &args, &logged).await?;
  Ok(())
}

/// Pure argument builder for `sbx secret rm <service> [--sandbox <name>]
/// -f`, the documented, canonical removal form (see the `sbx secret rm`
/// CLI reference).
fn service_secret_rm_args(service: &str, sandbox_name: Option<&str>) -> Vec<String> {
  let mut args = vec!["secret".to_string(), "rm".to_string(), service.to_string()];
  if let Some(sbx) = sandbox_name {
    args.push("--sandbox".to_string());
    args.push(sbx.to_string());
  }
  args.push("-f".to_string());
  args
}

pub async fn remove_service_secret<R: Runtime>(app: &AppHandle<R>, service: &str, sandbox_name: Option<&str>) -> Result<()> {
  let args = service_secret_rm_args(service, sandbox_name);
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  run_with_logged_args_timed(app, "Remove service secret", &args, &args).await?;
  Ok(())
}

/// Pure argument builder for `sbx secret rm [--sandbox <name>] --host
/// <host> -f` — removal for a custom secret.
///
/// **UNVERIFIED against a real `sbx` install.** The canonical `sbx
/// secret rm` reference documents `rm [SERVICE] [flags]` with no
/// `--host` flag at all; a separate Docker guide
/// (`customize/build-an-agent.md`) demonstrates exactly this form to
/// remove a `set-custom` entry, explicitly noting `--host` "doesn't
/// appear in `sbx secret rm --help`" — i.e. real but hidden. If a real
/// `sbx secret rm --help` proves this wrong, this is the only function
/// that needs to change; every caller goes through it.
fn custom_secret_rm_args(host: &str, sandbox_name: Option<&str>) -> Vec<String> {
  let mut args = vec!["secret".to_string(), "rm".to_string()];
  if let Some(sbx) = sandbox_name {
    args.push("--sandbox".to_string());
    args.push(sbx.to_string());
  }
  args.push("--host".to_string());
  args.push(host.to_string());
  args.push("-f".to_string());
  args
}

pub async fn remove_custom_secret<R: Runtime>(app: &AppHandle<R>, host: &str, sandbox_name: Option<&str>) -> Result<()> {
  let args = custom_secret_rm_args(host, sandbox_name);
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  run_with_logged_args_timed(app, "Remove custom secret", &args, &args).await?;
  Ok(())
}

/// Health check for the Sandboxes page. `sbx ls` is read-only, so (unlike
/// `sbx run`/`sbx create`) it shouldn't trigger the interactive first-run
/// network-policy prompt — it errors cleanly if `sbx` isn't installed or
/// isn't logged in.
pub async fn health_check<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
  run(app, "Sandbox health check", &["ls"]).await?;
  Ok(())
}

/// `sbx create --name <name> [--clone] claude <workspace>` — creates a
/// sandbox in the background without attaching. `workspace` is the host
/// repo path; in clone mode `sbx` clones it into an isolated copy inside
/// the sandbox VM itself rather than us managing a host-side clone folder.
///
/// If this machine's global network policy has never been set, this
/// returns `Error::PolicyNotInitialized` — callers should prompt for a
/// preset and call `policy_init` before retrying.
pub async fn create<R: Runtime>(app: &AppHandle<R>, name: &str, clone: bool, workspace: &str) -> Result<()> {
  let mut args = vec!["create", "--name", name];
  if clone {
    args.push("--clone");
  }
  args.push("claude");
  args.push(workspace);
  run(app, "Create sandbox", &args).await?;
  Ok(())
}

/// Makes a bare `claude` typed inside a manually-opened terminal
/// (`open_sandbox_terminal`'s `sbx exec -it <name> bash`) use the same
/// permission mode the sandbox was created with, not just app-launched
/// autonomous sessions.
///
/// `sbx run --name <name> claude` goes through sbx's own managed `claude`
/// agent entrypoint and its own default startup flags. But our terminal
/// button drops into a bare shell instead (so the user can run arbitrary
/// commands, not just attach to the agent), and a `claude` typed there
/// invokes the raw binary with none of sbx's defaults — landing on Claude
/// Code's own manual-approval default regardless of what was configured.
/// Confirmed against a real sandbox: `/status` inside a manually-opened
/// terminal showed "manual" even though the app's own launched sessions
/// were passing a different mode explicitly.
///
/// Appending an alias to `/etc/sandbox-persistent.sh` (sourced for every
/// bash invocation, interactive or not, per sbx's docs) closes that gap.
/// It only needs to run once at creation — the file is part of the
/// sandbox's persistent state and survives stop/resume. `mode` must
/// already be one of Claude Code's valid `--permission-mode` values —
/// callers are expected to have validated it (see commands.rs's
/// VALID_PERMISSION_MODES).
pub async fn set_claude_default_permission_mode<R: Runtime>(app: &AppHandle<R>, name: &str, mode: &str) -> Result<()> {
  run(
    app,
    "Set default Claude permission mode",
    &[
      "exec",
      "-d",
      name,
      "bash",
      "-c",
      &format!("echo \"alias claude='claude --permission-mode {mode}'\" >> /etc/sandbox-persistent.sh"),
    ],
  )
  .await?;
  Ok(())
}

/// `sbx exec -d <name> git config --global <key> <value>` — pushes one
/// host git identity field into a sandbox. `sbx` doesn't import host
/// `$HOME` config, so a fresh sandbox otherwise has none. This write is
/// immediate and idempotent, unlike `set_claude_default_permission_mode`'s
/// persistent-shell-file trick.
pub async fn set_git_config<R: Runtime>(app: &AppHandle<R>, name: &str, key: &str, value: &str) -> Result<()> {
  let args = git_config_exec_args(name, key, value);
  run(app, "Set sandbox git config", &args.iter().map(String::as_str).collect::<Vec<_>>()).await?;
  Ok(())
}

fn git_config_exec_args(name: &str, key: &str, value: &str) -> Vec<String> {
  ["exec", "-d", name, "git", "config", "--global", key, value].map(String::from).to_vec()
}

const ENV_BLOCK_START: &str = "# overnight-env-start";
const ENV_BLOCK_END: &str = "# overnight-env-end";

/// Builds the `bash -c` script that rewrites the app-managed env-var block
/// inside `file_path` (always `/etc/sandbox-persistent.sh` in production —
/// parameterized only so tests can point it at a temp file). Pure and
/// side-effect-free.
///
/// Rewrites via `awk` into a shell variable, then a single truncating `>`
/// write — not `sed -i`. `sed -i` creates its own temp file in the
/// *directory* containing the target and renames it into place, which
/// needs write permission on that directory. Confirmed against a real
/// `sbx` sandbox: `/etc` isn't writable by the sandbox user even though
/// `/etc/sandbox-persistent.sh` itself is, so `sed -i` failed with
/// "couldn't open temporary file /etc/sedXXXXXX: Permission denied". A
/// truncating `>` only needs write permission on the existing file,
/// matching the `>>` append `set_claude_default_permission_mode` already
/// relies on.
///
/// Every line is written via `printf '%s\n' <quoted>` instead of a heredoc.
/// The line that actually lands in the file (`export KEY='value'`) is
/// built with `shell_quote` so the *value* survives that file being
/// `source`d later unharmed; the whole line is then `shell_quote`d a
/// second time so it survives being passed as one argument to `printf` in
/// *this* script. Nesting `shell_quote` twice is the standard way to carry
/// a value through two levels of shell parsing safely, regardless of what
/// characters it contains.
fn build_env_persist_script(vars: &[EnvVar], file_path: &str) -> String {
  let quoted_path = shell_quote(file_path);

  let mut script = format!(
    "old=$(awk '/^{start}$/{{skip=1}} skip{{if(/^{end}$/){{skip=0}}; next}} {{print}}' {path} 2>/dev/null)\n",
    start = ENV_BLOCK_START,
    end = ENV_BLOCK_END,
    path = quoted_path,
  );
  script.push_str("{\n");
  script.push_str("  [ -n \"$old\" ] && printf '%s\\n' \"$old\"\n");
  script.push_str(&format!("  printf '%s\\n' {}\n", shell_quote(ENV_BLOCK_START)));
  for var in vars {
    let line = format!("export {}={}", var.key, shell_quote(&var.value));
    script.push_str(&format!("  printf '%s\\n' {}\n", shell_quote(&line)));
  }
  script.push_str(&format!("  printf '%s\\n' {}\n", shell_quote(ENV_BLOCK_END)));
  script.push_str(&format!("}} > {}", quoted_path));
  script
}

/// Replaces every value with a placeholder, keeping keys intact — used to
/// build the version of the persist script that's safe to write to the
/// command log (see `set_env_vars`). Keys stay visible: useful for "was
/// FOO ever set on this sandbox?" without exposing what it was set to.
fn redact_env_vars(vars: &[EnvVar]) -> Vec<EnvVar> {
  vars.iter().map(|v| EnvVar { key: v.key.clone(), value: "********".to_string() }).collect()
}

/// Rewrites the sandbox's persisted environment variables. `vars` is
/// expected to already be the merged result of global + project +
/// sandbox-scoped vars (see commands.rs::merged_env_vars) — this function
/// doesn't know about scopes, it just writes what it's given. Idempotent:
/// safe to call again after edits or removals, unlike
/// `set_claude_default_permission_mode`'s one-time append.
pub async fn set_env_vars<R: Runtime>(app: &AppHandle<R>, name: &str, vars: &[EnvVar]) -> Result<()> {
  let script = build_env_persist_script(vars, "/etc/sandbox-persistent.sh");
  // The command log must never carry real values — build the same script
  // with every value redacted, purely for logging (never executed).
  let redacted_script = build_env_persist_script(&redact_env_vars(vars), "/etc/sandbox-persistent.sh");
  run_with_logged_args(
    app,
    "Set sandbox environment variables",
    &["exec", "-d", name, "bash", "-c", &script],
    &["exec", "-d", name, "bash", "-c", &redacted_script],
  )
  .await?;
  Ok(())
}

/// One persisted "branch snapshot" — the sandbox's currently checked-out
/// branch, its full local branch list, and its worktrees, all captured
/// together by `read_branch_snapshot` in a single `sbx exec` round trip so
/// the three always agree with each other as of the same instant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchSnapshot {
  pub current_branch: Option<String>,
  pub branches: Vec<String>,
  pub worktrees: Vec<WorktreeInfo>,
}

/// Wraps `value` in single quotes for a POSIX `sh -c "..."` script,
/// escaping any embedded single quote (`'` -> `'\''`) — used for the
/// in-sandbox workspace path passed to every `git -C` invocation below,
/// which may contain spaces.
fn shell_quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', "'\\''"))
}

/// The sandbox home `sbx`'s docs land a bare attach in — used as
/// `read_branch_snapshot`'s fallback when no workspace path is known (e.g.
/// `sbx ls` had no WORKSPACE column for this sandbox), mirroring
/// `expand_home`'s definition of the same path.
const SANDBOX_HOME: &str = "/home/agent";

/// `sbx exec -d <name> sh -c "git -C <ws> rev-parse --abbrev-ref HEAD;
/// echo ---; git -C <ws> for-each-ref --format='%(refname:short)'
/// refs/heads; echo ---; git -C <ws> worktree list --porcelain"` — reads
/// the sandbox's current branch, full local branch list, and worktrees in
/// one round trip, so a caller (the Branch tab's live load, or a
/// best-effort read right before `stop`) only pays for one `sbx exec`
/// rather than three.
///
/// Deliberately bypasses this module's shared `run()` helper: `run()`
/// discards stdout on a non-zero exit, but a non-git `workspace_path` (or
/// any single failing git subcommand in the chain) makes the *whole*
/// `sh -c` script exit non-zero even though the commands before the
/// failure still produced usable output — so this reads `output.stdout`
/// unconditionally and lets `parse_branch_snapshot` degrade missing
/// sections to empty/`None` rather than erroring the whole call. Only a
/// real spawn/IO failure (`sbx` itself missing, etc.) surfaces as `Err`.
///
/// **UNVERIFIED**: no real `sbx` install is available in this dev
/// environment (see this module's top doc comment), so this exact
/// multi-command `sh -c` chaining and output shape is unconfirmed against
/// a real sandbox.
pub async fn read_branch_snapshot<R: Runtime>(
  app: &AppHandle<R>,
  name: &str,
  workspace_path: Option<&str>,
) -> Result<BranchSnapshot> {
  let ws = shell_quote(workspace_path.unwrap_or(SANDBOX_HOME));
  let script = format!(
    "git -C {ws} rev-parse --abbrev-ref HEAD; echo ---; \
     git -C {ws} for-each-ref --format='%(refname:short)' refs/heads; echo ---; \
     git -C {ws} worktree list --porcelain"
  );
  let exec_args = ["exec", "-d", name, "sh", "-c", &script];
  let output = app.shell().command("sbx").args(exec_args).output().await?;
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
  record(
    app,
    "Read sandbox branch snapshot",
    &exec_args,
    output.status.success(),
    output.status.code().map(i64::from),
    (!stderr.is_empty()).then_some(stderr.as_str()),
  );
  Ok(parse_branch_snapshot(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_branch_snapshot(output: &str) -> BranchSnapshot {
  let [head_section, branches_section, worktrees_section] = split_into_sections(output);

  let current_branch = match head_section.trim() {
    "" | "HEAD" => None, // empty (failed/non-git) or detached HEAD
    branch => Some(branch.to_string()),
  };
  let branches =
    branches_section.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect();
  let worktrees = parse_worktrees(&worktrees_section);

  BranchSnapshot { current_branch, branches, worktrees }
}

/// Splits `read_branch_snapshot`'s combined output on lines that are
/// exactly `---` (from the script's own `echo ---` separators) into
/// exactly 3 sections — padding with empty strings if a git failure meant
/// fewer than 2 separators ever printed.
fn split_into_sections(output: &str) -> [String; 3] {
  let mut sections = vec![String::new()];
  for line in output.lines() {
    if line == "---" {
      sections.push(String::new());
      continue;
    }
    let current = sections.last_mut().expect("sections always has at least one entry");
    if !current.is_empty() {
      current.push('\n');
    }
    current.push_str(line);
  }
  sections.resize(3, String::new());
  [sections[0].clone(), sections[1].clone(), sections[2].clone()]
}

/// Parses `git worktree list --porcelain` output: blocks separated by a
/// blank line, each a run of `key value` lines (`worktree <path>`, `HEAD
/// <sha>`, `branch refs/heads/<name>`, or bare `detached`/`bare`).
fn parse_worktrees(output: &str) -> Vec<WorktreeInfo> {
  output
    .split("\n\n")
    .filter_map(|block| {
      let mut path = None;
      let mut head_sha = String::new();
      let mut branch = None;
      for line in block.lines() {
        if let Some(value) = line.strip_prefix("worktree ") {
          path = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("HEAD ") {
          head_sha = value.to_string();
        } else if let Some(value) = line.strip_prefix("branch ") {
          branch = Some(value.trim_start_matches("refs/heads/").to_string());
        }
      }
      path.map(|path| WorktreeInfo { path, branch, head_sha })
    })
    .collect()
}

pub async fn stop<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<()> {
  run(app, "Stop sandbox", &["stop", name]).await?;
  Ok(())
}

/// Resumes a stopped sandbox. `sbx` has no plain "start" — re-running
/// `sbx create` against an existing name errors ("already exists, use
/// sbx run --name ... to connect"), confirming `sbx run --name <name>` is
/// the actual reconnect path. `sbx run` attaches interactively (starts the
/// VM if needed, then attaches to the agent session), so this spawns it
/// without awaiting completion rather than using `run()`'s `.output()` —
/// otherwise this call would block until the agent session ends. The
/// spawned process is intentionally not consumed further; dropping the
/// handle doesn't kill it; it keeps the VM (and its agent session) running
/// in the background the same way `sbx run` would from a terminal.
pub fn resume<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<()> {
  let args = vec!["run".to_string(), "--name".to_string(), name.to_string()];
  let log_app = app.clone();
  let log_args = args.clone();
  crate::process::spawn(app, "sbx", &args, None, move |success, code, stderr| {
    record(&log_app, "Resume sandbox", &log_args.iter().map(String::as_str).collect::<Vec<_>>(), success, code.map(i64::from), stderr.as_deref());
  })?;
  Ok(())
}

/// `sbx cp <name>:<remote_path> <host_dest>` — copies a file or directory
/// out of an isolated sandbox VM onto the host. Unlike `workspace_path`'s
/// WORKSPACE column (a mount-mode sandbox's path is already host-visible)
/// or branch 8's `/proc` reads (readable via `sbx exec`), there's no other
/// documented way to reach an arbitrary in-VM path from the host — `sbx cp`
/// is the one command built for exactly that, so this is a direct,
/// unwrapped shell-out rather than a thin layer over something else.
///
/// **CONFIRMED against a real sandbox** (docker-cp-style semantics): if
/// `host_dest` already exists as a directory, the copy nests
/// `remote_path`'s basename one level inside it instead of copying its
/// contents directly — e.g. copying `.git` onto an existing `host_dest`
/// lands at `host_dest/.git/...`, not `host_dest/...`. Callers that want
/// `remote_path`'s *contents* placed directly at `host_dest` must leave
/// `host_dest` non-existent (only its parent needs to exist) before
/// calling this, so `sbx cp` creates it fresh from the source.
pub async fn cp_from_sandbox<R: Runtime>(app: &AppHandle<R>, name: &str, remote_path: &str, host_dest: &str) -> Result<()> {
  let args = cp_from_sandbox_args(name, remote_path, host_dest);
  run(app, "Copy files from sandbox", &args.iter().map(String::as_str).collect::<Vec<_>>()).await?;
  Ok(())
}

fn cp_from_sandbox_args(name: &str, remote_path: &str, host_dest: &str) -> Vec<String> {
  vec!["cp".to_string(), format!("{name}:{remote_path}"), host_dest.to_string()]
}

/// `sbx cp <host_src> <name>:<remote_path>` — copies a file or directory
/// from the host into a running sandbox VM. Symmetric to `cp_from_sandbox`.
/// Low-level primitive: prefer `restore_directory` for restoring a backup
/// onto a path that already exists in the sandbox, since this nests
/// unpredictably in that case (see its doc comment).
///
/// **CONFIRMED against a real sandbox, twice over**: nests one level
/// whether or not `remote_path` already exists — first observed nesting
/// `host_src`'s basename under an existing `remote_path`
/// (`.git` restored onto `.git` produced `.git/git/...`); after removing
/// `remote_path` first, it *still* nested — this time reproducing
/// `remote_path`'s own basename as the child (`.git` → `.git/.git/...`).
/// The exact rule isn't pinned down and may not be stable across `sbx`
/// versions, so `restore_directory` treats the nesting depth as unknown
/// and resolves it generically rather than assuming either shape.
pub async fn cp_to_sandbox<R: Runtime>(app: &AppHandle<R>, name: &str, host_src: &str, remote_path: &str) -> Result<()> {
  let args = cp_to_sandbox_args(name, host_src, remote_path);
  run(app, "Copy files to sandbox", &args.iter().map(String::as_str).collect::<Vec<_>>()).await?;
  Ok(())
}

fn cp_to_sandbox_args(name: &str, host_src: &str, remote_path: &str) -> Vec<String> {
  vec!["cp".to_string(), host_src.to_string(), format!("{name}:{remote_path}")]
}

/// `~/.claude` subdirectories that are their own mounted volumes rather
/// than plain directories — confirmed against a real sandbox: each one has
/// its own `lost+found` (only present at a filesystem's own root), and
/// `rm -rf ~/.claude` failed on exactly these six with "Device or resource
/// busy" while every plain file/directory alongside them removed cleanly.
/// You cannot remove or replace a mount point itself — but this is also
/// where conversation history (`claude -r` reads from `sessions`/`projects`)
/// actually lives, so a `.claude` restore can't just skip them either.
/// `restore_directory` clears and refills these in place instead of
/// removing them outright. A plain additive merge (copy the backup's files
/// in, leave whatever was already there) isn't equivalent to a real
/// restore either: confirmed against a real sandbox that a full manual
/// swap of the whole `.claude` folder behaved differently from our
/// merge-only restore, meaning stale target-side leftovers coexisting with
/// the backup's files inside these directories can itself cause problems —
/// so the fix is to clear each one's contents before copying the backup in,
/// not just layer on top.
pub const CLAUDE_HOME_MOUNTED_DIRS: &[&str] = &["projects", "sessions", "shell-snapshots", "statsig", "todos", "skills"];

/// Restores `host_src` into `remote_dest` inside the sandbox by merging one
/// top-level entry at a time — never removing or replacing `remote_dest`
/// itself. Most entries are fully replaced (remove that name under
/// `remote_dest`, move the backup's version into its place); entries named
/// in `merge_in_place` (`~/.claude`'s `CLAUDE_HOME_MOUNTED_DIRS`, which are
/// mount points and can't be removed or replaced as a whole) instead get
/// their *contents* cleared and then replaced with the backup's, so it's a
/// real replace even though the mount point directory entry itself is
/// never touched — restoring `.claude` still brings back conversation
/// history in `sessions`/`projects` rather than skipping it, but without
/// leaving stale target-side files mixed in with the restored ones. Copies
/// into a scratch path first, then (one `sbx exec`) walks down through any
/// wrapper directories that hold exactly one child before merging —
/// regardless of how many levels deep `cp_to_sandbox` happens to nest it. A
/// real backup (`.git`, `~/.claude`) always has multiple top-level entries,
/// so this can't walk past the actual content into it by mistake.
pub async fn restore_directory<R: Runtime>(
  app: &AppHandle<R>,
  name: &str,
  host_src: &str,
  remote_dest: &str,
  merge_in_place: &[&str],
) -> Result<()> {
  let scratch = format!("/tmp/overnight-restore-{}", crate::db::models::new_id());
  cp_to_sandbox(app, name, host_src, &scratch).await?;
  let script = restore_directory_script(&scratch, remote_dest, merge_in_place);
  run(app, "Restore backup into sandbox", &["exec", "-d", name, "sh", "-c", &script]).await?;
  Ok(())
}

/// Pure POSIX globbing rather than `find -mindepth/-maxdepth` — a sandbox's
/// shell is minimal (BusyBox-ish) and those flags aren't a safe assumption;
/// a shell lacking them would silently make the whole loop a no-op instead
/// of erroring, which is exactly the failure this is guarding against.
/// `[ -e "$f" ] || [ -L "$f" ]` filters out glob patterns that matched
/// nothing (passed through literally when nothing matches) — the standard
/// portable way to detect an empty match without `nullglob`.
///
/// `merge_in_place` entries never go through `rm -rf`/`mv` on the entry
/// itself (which would fail on a mount point) — instead their *contents*
/// get cleared (`rm -rf dest/name/*` and hidden variants, best-effort: a
/// stray root-owned `lost+found` may survive that, harmless) and then
/// `cp -a "$entry"/. dest/name/` copies the backup's files in fresh. That
/// two-step gives real replace semantics without ever removing the mount
/// point directory entry itself.
///
/// The final cleanup (`rm -rf` the scratch copy) is deliberately
/// best-effort: a mount-backed directory still gets copied into scratch
/// wholesale (there's no way to `sbx cp` just its contents), and its
/// contents can carry permissions (root-owned `lost+found`, etc.) our exec
/// user can't remove — that must never fail the restore itself, since the
/// actual merge above it already succeeded.
///
/// Tracks a `fail` flag across every entry instead of ending on a bare
/// `true`: a script that always exits 0 regardless of what happened inside
/// means a `mv`/`cp -a` that silently fails partway through (permissions,
/// a busy mount, anything) is invisible to `run()`'s exit-status check —
/// the restore gets reported as fully successful even though an entry
/// never made it across. Only the genuinely best-effort steps (clearing a
/// mount-backed dir's old contents, and the final scratch cleanup) stay
/// swallowed; the actual data-carrying copy of every entry, mount-backed
/// or not, now fails the whole script if it fails.
///
/// Plain entries try `mv` first, falling back to `cp -a` (source left
/// behind for the final scratch cleanup to sweep up) rather than treating
/// an `mv` failure as fatal outright. **Confirmed against a real
/// sandbox**: `~/.claude` itself was owned by the exec user with normal
/// `755` permissions, yet every `mv` of a plain top-level entry into it
/// failed with "Permission denied" — while `cp -a` into the mount-backed
/// subdirectories in that same run succeeded. That split (a `rename`-style
/// move rejected, a plain copy accepted, on a directory the user
/// demonstrably owns and can write to) points at the backing filesystem
/// refusing `rename(2)` specifically rather than an actual permissions
/// problem, a known limitation of some virtiofs/9p-style microVM home
/// directories. `cp -a` uses `open`/`write` instead, so it isn't affected.
fn restore_directory_script(scratch: &str, remote_dest: &str, merge_in_place: &[&str]) -> String {
  let scratch_q = shell_quote(scratch);
  let dest_q = shell_quote(remote_dest);
  let merge_case = if merge_in_place.is_empty() {
    String::new()
  } else {
    format!(
      "case \"$name\" in {names}) \
         if [ -d \"$entry\" ]; then \
           mkdir -p {dest_q}/\"$name\" || fail=1; \
           rm -rf {dest_q}/\"$name\"/.[!.]* {dest_q}/\"$name\"/..?* {dest_q}/\"$name\"/* 2>/dev/null; \
           cp -a \"$entry\"/. {dest_q}/\"$name\"/ || fail=1; \
           continue; \
         fi ;; \
       esac; ",
      names = merge_in_place.join("|")
    )
  };
  format!(
    "src={scratch_q}; \
     fail=0; \
     while true; do \
       n=0; only=''; \
       for f in \"$src\"/.[!.]* \"$src\"/..?* \"$src\"/*; do \
         [ -e \"$f\" ] || [ -L \"$f\" ] || continue; \
         n=$((n + 1)); \
         only=\"$f\"; \
       done; \
       [ \"$n\" -eq 1 ] || break; \
       [ -d \"$only\" ] || break; \
       src=\"$only\"; \
     done; \
     mkdir -p {dest_q}; \
     for entry in \"$src\"/.[!.]* \"$src\"/..?* \"$src\"/*; do \
       [ -e \"$entry\" ] || [ -L \"$entry\" ] || continue; \
       name=${{entry##*/}}; \
       {merge_case}\
       rm -rf {dest_q}/\"$name\" || fail=1; \
       mv \"$entry\" {dest_q}/\"$name\" 2>/dev/null || cp -a \"$entry\" {dest_q}/\"$name\" || fail=1; \
     done; \
     rm -rf {scratch_q} 2>/dev/null; \
     exit $fail"
  )
}

/// Force-removes the sandbox and its VM (used by explicit sandbox Delete).
pub async fn rm<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<()> {
  run(app, "Remove sandbox", &["rm", "--force", name]).await?;
  Ok(())
}

/// Publishes `sandbox_port` on an OS-assigned host port. Use `host_port`
/// afterward to find out which port was assigned.
pub async fn publish_port<R: Runtime>(app: &AppHandle<R>, name: &str, sandbox_port: u16) -> Result<()> {
  run(app, "Publish sandbox port", &["ports", name, "--publish", &sandbox_port.to_string()]).await?;
  Ok(())
}

/// Parses `sbx ports <name>` output for the host port mapped to
/// `sandbox_port`. Expected line shape (per docs):
/// `127.0.0.1:8080->3000/tcp` — no confirmed `--format json`, so this is a
/// best-effort regex-free parse of that pattern.
pub async fn host_port<R: Runtime>(app: &AppHandle<R>, name: &str, sandbox_port: u16) -> Result<Option<u16>> {
  let output = run(app, "List sandbox ports", &["ports", name]).await?;
  Ok(parse_host_port(&output, sandbox_port))
}

fn parse_host_port(output: &str, sandbox_port: u16) -> Option<u16> {
  let suffix = format!("->{sandbox_port}/tcp");
  output.lines().find_map(|line| {
    let mapping = line.split_whitespace().find(|token| token.contains(&suffix))?;
    let host_part = mapping.strip_suffix(&suffix)?;
    host_part.rsplit(':').next()?.parse().ok()
  })
}

/// Looks up the in-VM workspace path `sbx` reports for `name` via `sbx ls`'s
/// WORKSPACE column — the same path a plain `sbx exec`/`sbx run` attach
/// lands you in by default (confirmed against real output: terminal opens
/// there with no path argument of our own). On Windows, `sbx ls` echoes
/// this back in raw Windows form (`F:\works\personal\remind-me`) even for
/// a clone-mode sandbox — confirmed by a failed VS Code connection whose
/// error message quoted that exact unmodified path — so it still needs
/// the same drive-letter-to-POSIX translation a mount-mode host path
/// would, regardless of mode.
pub async fn workspace_path<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<Option<String>> {
  let output = run(app, "List sandboxes", &["ls"]).await?;
  Ok(parse_workspace_path(&output, name).map(|path| expand_home(&windows_path_to_posix(&path))))
}

/// Converts a Windows path (`F:\works\personal\remind-me`) to the POSIX
/// form sbx/ssh use inside the sandbox VM (`/f/works/personal/remind-me`).
/// A no-op on any path that isn't already in that drive-letter form (e.g.
/// already-POSIX paths from a non-Windows host, or `~`-relative ones).
pub(crate) fn windows_path_to_posix(path: &str) -> String {
  let mut chars = path.chars();
  match (chars.next(), chars.next()) {
    (Some(drive), Some(':')) if drive.is_ascii_alphabetic() => {
      let rest = chars.as_str().replace('\\', "/");
      let rest = rest.strip_prefix('/').unwrap_or(&rest);
      format!("/{}/{rest}", drive.to_ascii_lowercase())
    }
    _ => path.to_string(),
  }
}

fn parse_workspace_path(output: &str, name: &str) -> Option<String> {
  output.lines().find_map(|line| {
    let mut tokens = line.split_whitespace();
    if tokens.next()? != name {
      return None;
    }
    tokens.last().map(str::to_string)
  })
}

/// `sbx ls` can report the agent's home-relative shorthand (`~` or
/// `~/my-project`, per the docs) instead of an absolute path. URIs don't
/// do shell tilde-expansion, so this expands it against the documented
/// sandbox home (`/home/agent`) before it's used to build a folder URI.
fn expand_home(path: &str) -> String {
  match path.strip_prefix('~') {
    Some(rest) => format!("/home/agent{rest}"),
    None => path.to_string(),
  }
}

/// One row of `sbx ls` output, for callers that need every sandbox `sbx`
/// knows about rather than looking up a single name (see `list_all`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbxListRow {
  pub sbx_name: String,
  pub status: String,
  /// Same POSIX-normalized/`~`-expanded form `workspace_path` returns for a
  /// single lookup — `None` when the row has no WORKSPACE column value to
  /// parse (should be rare; degrades gracefully rather than erroring).
  pub workspace_path: Option<String>,
}

/// Lists every sandbox `sbx ls` reports, not just one by name — generalizes
/// `parse_sandbox_status`/`parse_workspace_path`'s single-row lookups for
/// the orphan-adoption check, which needs to compare *all* rows against the
/// app's DB at once.
///
/// **UNVERIFIED**: same caveat as `parse_policy_rules` — no real `sbx`
/// install is available in this dev environment, so it's unconfirmed
/// whether `sbx ls` lists stopped sandboxes alongside running ones (the
/// orphan-adoption feature only works for stopped sandboxes too if it
/// does). The parser itself degrades to an empty list rather than
/// misparsing if the real layout differs.
pub async fn list_all<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<SbxListRow>> {
  let output = run(app, "List sandboxes", &["ls"]).await?;
  Ok(
    parse_sandbox_rows(&output)
      .into_iter()
      .map(|row| SbxListRow {
        workspace_path: row.workspace_path.map(|path| expand_home(&windows_path_to_posix(&path))),
        ..row
      })
      .collect(),
  )
}

/// Parses every data row from `sbx ls` output using the same fixed-position
/// layout `parse_sandbox_status`/`parse_workspace_path` already assume
/// (`SANDBOX AGENT STATUS [PORTS] WORKSPACE`): first token is the name,
/// third is status, last is the workspace path — but only when there are
/// more than 3 tokens, so a row with no workspace column doesn't misread
/// its own status token as a path.
///
/// Requires seeing a real header line first (first non-empty line whose
/// first token is literally "SANDBOX", case-insensitive) before treating
/// anything as a data row. This alone isn't enough, though: `sbx ls` prints
/// that header *unconditionally*, even with zero sandboxes, and swaps only
/// the table *body* for a decorated empty-state box (confirmed by a
/// corrupted DB row this misparse produced in the wild — a fabricated
/// sandbox with status "Sandboxes" and sbx_name "│", read straight off the
/// box's "No Sandboxes found." line). So every line is also rejected if it
/// contains a box-drawing character (U+2500-U+257F) — never legal in a
/// sandbox name, status, or workspace path — which catches the box's
/// borders and content regardless of whether a header preceded it. If no
/// header is ever found, this isn't a table at all, so this degrades to an
/// empty Vec rather than misparsing.
fn parse_sandbox_rows(output: &str) -> Vec<SbxListRow> {
  fn is_box_drawing(line: &str) -> bool {
    line.chars().any(|c| ('\u{2500}'..='\u{257f}').contains(&c))
  }

  let mut lines = output.lines().filter(|line| !line.trim().is_empty() && !is_box_drawing(line));

  let saw_header = lines
    .by_ref()
    .any(|line| line.split_whitespace().next().is_some_and(|t| t.eq_ignore_ascii_case("SANDBOX")));
  if !saw_header {
    return Vec::new();
  }

  lines
    .filter_map(|line| {
      let tokens: Vec<&str> = line.split_whitespace().collect();
      if tokens.len() < 3 || tokens[0].eq_ignore_ascii_case("SANDBOX") {
        return None;
      }
      let sbx_name = tokens[0].to_string();
      let status = tokens[2].to_string();
      let workspace_path = (tokens.len() > 3).then(|| tokens[tokens.len() - 1].to_string());
      Some(SbxListRow { sbx_name, status, workspace_path })
    })
    .collect()
}

/// Extracts the STATUS column for `name` from `sbx ls` output (same table
/// shape as `parse_workspace_path`/`parse_host_port`).
fn parse_sandbox_status(output: &str, name: &str) -> Option<String> {
  output.lines().find_map(|line| {
    let mut tokens = line.split_whitespace();
    if tokens.next()? != name {
      return None;
    }
    tokens.next()?; // AGENT column
    tokens.next().map(str::to_string) // STATUS column
  })
}

/// One-shot `sbx ls` read of `name`'s current STATUS column — `None` if
/// `name` isn't reported at all. Callers reconcile the DB with this after
/// any action that can change a sandbox's state as a side effect without
/// its own dedicated event (e.g. a network-policy change that cuts off the
/// sandbox's connectivity and stops it — confirmed against a real `sbx`
/// install for a per-sandbox "Locked Down" override).
pub async fn current_status<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<Option<String>> {
  let output = run(app, "List sandboxes", &["ls"]).await?;
  Ok(parse_sandbox_status(&output, name))
}

/// Polls `sbx ls` until `name` reports STATUS "running", rather than a
/// caller assuming a sandbox is usable the instant a prior call (e.g.
/// `create`) returns. Standalone so any caller needing this same readiness
/// gate (e.g. a future resume/start path) can reuse it without depending
/// on create/stop-specific state.
pub async fn wait_until_ready<R: Runtime>(
  app: &AppHandle<R>,
  name: &str,
  timeout: std::time::Duration,
  poll_interval: std::time::Duration,
) -> Result<()> {
  let deadline = std::time::Instant::now() + timeout;
  loop {
    let output = run(app, "List sandboxes", &["ls"]).await?;
    if parse_sandbox_status(&output, name).as_deref() == Some("running") {
      return Ok(());
    }
    if std::time::Instant::now() >= deadline {
      return Err(Error::CommandFailed(format!("sandbox {name} not ready after {timeout:?}")));
    }
    tokio::time::sleep(poll_interval).await;
  }
}

/// `sbx run --name <name> claude -- <claude-args>` — attaches to (and if
/// needed starts) the agent session. Args after `--` are appended to
/// Claude Code's default startup flags per the sbx docs, so passing
/// `-p <prompt> --output-format stream-json ...` here preserves the same
/// stream-json event pipeline the host-run path used before sandboxes.
/// This runs as a foreground attach — for a one-shot `-p` invocation the
/// underlying `claude` process exits on its own, ending the attach and
/// closing the stdout stream like any other spawned process.
pub fn run_agent<R: Runtime>(app: &AppHandle<R>, name: &str, claude_args: &[String]) -> Result<SpawnedProcess> {
  let mut args = vec!["run".to_string(), "--name".to_string(), name.to_string(), "claude".to_string(), "--".to_string()];
  args.extend_from_slice(claude_args);
  let log_app = app.clone();
  let log_args = args.clone();
  Ok(crate::process::spawn(app, "sbx", &args, None, move |success, code, stderr| {
    record(&log_app, "Run agent session", &log_args.iter().map(String::as_str).collect::<Vec<_>>(), success, code.map(i64::from), stderr.as_deref());
  })?)
}

/// Free host RAM in megabytes, for the pre-create memory heuristic. sbx
/// sandboxes are full microVMs (heavier than a plain container), so this
/// stays a soft client-side guard rather than a limit enforced by `sbx`
/// itself — no `--memory`/`--cpus` flag is documented on `sbx create`/`run`.
pub fn host_free_memory_mb() -> f64 {
  let mut sys = sysinfo::System::new();
  sys.refresh_memory();
  sys.available_memory() as f64 / (1024.0 * 1024.0)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HostStats {
  pub cpu_percent: f64,
  pub memory_percent: f64,
  pub memory_used_mb: f64,
  pub memory_total_mb: f64,
  /// Aggregate space usage across every mounted disk sysinfo can see —
  /// there's no cross-platform disk I/O throughput API in sysinfo, so this
  /// is capacity, not activity (it changes slowly, unlike the other stats).
  pub disk_percent: f64,
  pub disk_used_mb: f64,
  pub disk_total_mb: f64,
  pub network_rx_kb_per_sec: f64,
  pub network_tx_kb_per_sec: f64,
}

/// Holds the long-lived state `sample_host_stats` needs to reuse across
/// polls: `sysinfo` computes both CPU usage and network throughput as a
/// delta between two refreshes of the *same* instance, so recreating
/// these each call would always report zero. Managed as Tauri app state.
pub struct HostMonitor {
  system: sysinfo::System,
  networks: sysinfo::Networks,
  last_sampled: Option<std::time::Instant>,
}

impl HostMonitor {
  pub fn new() -> Self {
    Self {
      system: sysinfo::System::new_all(),
      networks: sysinfo::Networks::new_with_refreshed_list(),
      last_sampled: None,
    }
  }
}

/// Samples current host-wide CPU/memory/disk/network usage. See
/// `HostMonitor`'s doc comment for why `monitor` must be reused across
/// calls rather than recreated. Polled every 5s from the frontend — well
/// above `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL` — so this yields a
/// meaningful reading on every call after the first.
pub fn sample_host_stats(monitor: &mut HostMonitor) -> HostStats {
  monitor.system.refresh_cpu_usage();
  monitor.system.refresh_memory();
  monitor.networks.refresh();

  let memory_total_mb = monitor.system.total_memory() as f64 / (1024.0 * 1024.0);
  let memory_used_mb = monitor.system.used_memory() as f64 / (1024.0 * 1024.0);
  let memory_percent = if memory_total_mb > 0.0 { memory_used_mb / memory_total_mb * 100.0 } else { 0.0 };

  let disks = sysinfo::Disks::new_with_refreshed_list();
  let (disk_total, disk_available) = disks
    .list()
    .iter()
    .fold((0u64, 0u64), |(total, available), d| (total + d.total_space(), available + d.available_space()));
  let disk_total_mb = disk_total as f64 / (1024.0 * 1024.0);
  let disk_used_mb = (disk_total - disk_available) as f64 / (1024.0 * 1024.0);
  let disk_percent = if disk_total_mb > 0.0 { disk_used_mb / disk_total_mb * 100.0 } else { 0.0 };

  let elapsed_secs = monitor.last_sampled.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
  monitor.last_sampled = Some(std::time::Instant::now());
  let (rx_bytes, tx_bytes) = monitor
    .networks
    .list()
    .values()
    .fold((0u64, 0u64), |(rx, tx), data| (rx + data.received(), tx + data.transmitted()));
  let (network_rx_kb_per_sec, network_tx_kb_per_sec) = if elapsed_secs > 0.0 {
    (rx_bytes as f64 / 1024.0 / elapsed_secs, tx_bytes as f64 / 1024.0 / elapsed_secs)
  } else {
    (0.0, 0.0)
  };

  HostStats {
    cpu_percent: monitor.system.global_cpu_usage() as f64,
    memory_percent,
    memory_used_mb,
    memory_total_mb,
    disk_percent,
    disk_used_mb,
    disk_total_mb,
    network_rx_kb_per_sec,
    network_tx_kb_per_sec,
  }
}

/// What `SandboxMonitor` stores per sandbox: the previous CPU-jiffies and
/// cumulative network-byte counters, plus when they were read, so the
/// next sample can compute a CPU%/network-KB/s delta against them.
/// `memory_used_mb` deliberately isn't stored here — it's already a true
/// instantaneous value (not a rate), so `sample_resource_usage` never
/// needs a *previous* memory reading to report a *current* one.
#[derive(Debug, Clone, Copy)]
struct SandboxSample {
  cpu_total: u64,
  cpu_idle: u64,
  network_rx_bytes: u64,
  network_tx_bytes: u64,
  timestamp: Instant,
}

/// Holds the previous `/proc` sample for every sandbox that's been
/// polled, so `sample_resource_usage` can compute CPU%/network-KB/s as a
/// delta against each sandbox's own last reading — mirrors `HostMonitor`'s
/// reuse-across-polls role, but keyed by sandbox id since there can be
/// many sandboxes polled independently (only while each one's Metrics tab
/// is open — see `sample_resource_usage`'s doc comment, added alongside
/// this struct's first real user). Managed as Tauri app state
/// (`Mutex<SandboxMonitor>`), same as `HostMonitor`.
#[derive(Default)]
pub struct SandboxMonitor {
  samples: HashMap<String, SandboxSample>,
}

impl SandboxMonitor {
  pub fn new() -> Self {
    Self::default()
  }
}

/// `sbx exec -d <name> sh -c "..."` script backing `sample_resource_usage` —
/// reads the three `/proc` files a per-sandbox CPU/memory/network reading
/// needs in one round trip (host `docker stats` can't see inside a
/// sandbox's isolated microVM at all — see this module's top doc comment
/// and the Branch 8 planning note it's based on). `echo ---` separators
/// mirror `read_branch_snapshot`'s section-splitting trick so the same
/// `split_into_sections` helper parses this output too.
const RESOURCE_USAGE_SCRIPT: &str = "cat /proc/stat; echo ---; cat /proc/meminfo; echo ---; cat /proc/net/dev";

/// A single point-in-time reading of the raw (non-rate) counters
/// `sample_resource_usage` needs: cumulative CPU jiffies, current memory
/// used, and cumulative network byte counters. `cpu_percent` and the
/// network KB/s rates are only meaningful as a *delta* between two of
/// these (see `compute_delta_usage`) — `memory_used_mb` is the one field
/// that's already a true instantaneous value, not a rate.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct RawProcSample {
  cpu_total: u64,
  cpu_idle: u64,
  memory_used_mb: f64,
  network_rx_bytes: u64,
  network_tx_bytes: u64,
}

/// Parses the combined `RESOURCE_USAGE_SCRIPT` output into a `RawProcSample`.
/// Degrades missing/malformed sections to zeroed fields (same "partial
/// output is still usable" philosophy as `parse_branch_snapshot` — a
/// mid-script failure, or a totally empty read on a spawn error, shouldn't
/// crash the whole sample, just report zeros for what couldn't be read).
fn parse_raw_proc_sample(output: &str) -> RawProcSample {
  let [stat_section, meminfo_section, net_section] = split_into_sections(output);
  let (cpu_total, cpu_idle) = parse_cpu_jiffies(&stat_section).unwrap_or((0, 0));
  let memory_used_mb = parse_memory_used_mb(&meminfo_section);
  let (network_rx_bytes, network_tx_bytes) = parse_network_bytes(&net_section);
  RawProcSample { cpu_total, cpu_idle, memory_used_mb, network_rx_bytes, network_tx_bytes }
}

/// Parses `/proc/stat`'s aggregate `cpu` line (the one starting with the
/// bare token `cpu`, not `cpu0`/`cpu1`/...) into `(total_jiffies,
/// idle_jiffies)`. Per `man proc`, the fields after `cpu` are `user nice
/// system idle iowait irq softirq steal guest guest_nice` — `idle` is the
/// 4th field (index 3, 0-based after skipping the `cpu` label) and
/// `iowait` (index 4) counts as idle time too for a standard "busy %"
/// calculation. `total` sums every field present, tolerating kernels that
/// don't report the newer `guest`/`guest_nice` fields.
fn parse_cpu_jiffies(section: &str) -> Option<(u64, u64)> {
  let line = section.lines().find(|line| line.split_whitespace().next() == Some("cpu"))?;
  let fields: Vec<u64> = line.split_whitespace().skip(1).map(|f| f.parse().ok()).collect::<Option<Vec<u64>>>()?;
  if fields.len() < 4 {
    return None;
  }
  let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
  let total = fields.iter().sum();
  Some((total, idle))
}

/// Parses `/proc/meminfo`'s `MemTotal`/`MemAvailable` (or `MemFree` on
/// older kernels without `MemAvailable`) into used memory in MB, the same
/// "total minus available" definition `sample_host_stats` uses for the
/// host-wide reading.
fn parse_memory_used_mb(section: &str) -> f64 {
  let mut mem_total_kb: Option<u64> = None;
  let mut mem_available_kb: Option<u64> = None;
  let mut mem_free_kb: Option<u64> = None;
  for line in section.lines() {
    let mut parts = line.split_whitespace();
    let Some(key) = parts.next() else { continue };
    let Some(value) = parts.next().and_then(|v| v.parse::<u64>().ok()) else { continue };
    match key.trim_end_matches(':') {
      "MemTotal" => mem_total_kb = Some(value),
      "MemAvailable" => mem_available_kb = Some(value),
      "MemFree" => mem_free_kb = Some(value),
      _ => {}
    }
  }
  let total = mem_total_kb.unwrap_or(0);
  let available = mem_available_kb.or(mem_free_kb).unwrap_or(0);
  total.saturating_sub(available) as f64 / 1024.0
}

/// Parses `/proc/net/dev`'s per-interface table, summing RX/TX byte
/// counters across every interface except loopback (`lo` carries no real
/// network activity — including it would just add noise). Each interface
/// line is `<iface>: <8 RX fields> <8 TX fields>`; RX bytes is the first
/// field after the colon, TX bytes is the 9th (index 8) — the two header
/// lines (`Inter-|   Receive ...` / ` face |bytes ...`) have no colon and
/// are skipped naturally by `split_once(':')` returning `None`.
fn parse_network_bytes(section: &str) -> (u64, u64) {
  section
    .lines()
    .filter_map(|line| {
      let (iface, rest) = line.split_once(':')?;
      let iface = iface.trim();
      if iface.is_empty() || iface == "lo" {
        return None;
      }
      let fields: Vec<u64> = rest.split_whitespace().filter_map(|f| f.parse().ok()).collect();
      if fields.len() < 9 {
        return None;
      }
      Some((fields[0], fields[8]))
    })
    .fold((0u64, 0u64), |(rx, tx), (r, t)| (rx + r, tx + t))
}

/// One resource sample computed for a single sandbox: `cpu_percent` and
/// the network KB/s rates are deltas against the sandbox's previous
/// sample (see `compute_delta_usage`); `memory_mb` is always a true
/// instantaneous reading, delta or not.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct SandboxResourceUsage {
  pub cpu_percent: f64,
  pub memory_mb: f64,
  pub network_rx_kb_per_sec: f64,
  pub network_tx_kb_per_sec: f64,
}

/// Computes a `SandboxResourceUsage` delta from `previous` (this
/// sandbox's last stored sample, if any) to `current`. `now` is passed in
/// (rather than read internally) so this stays a pure function callers
/// can unit test with synthetic timestamps. No previous sample (first
/// poll for this sandbox) returns 0 for `cpu_percent` and both network
/// rates — there's nothing to compute a rate against yet — while
/// `memory_mb` still reports `current`'s true value since it isn't a
/// delta. `saturating_sub` throughout guards against a counter appearing
/// to go backwards (e.g. the sandbox restarted between polls, resetting
/// `/proc`'s counters) producing a nonsensical negative delta instead of
/// underflowing.
fn compute_delta_usage(previous: Option<&SandboxSample>, current: RawProcSample, now: Instant) -> SandboxResourceUsage {
  let Some(prev) = previous else {
    return SandboxResourceUsage {
      cpu_percent: 0.0,
      memory_mb: current.memory_used_mb,
      network_rx_kb_per_sec: 0.0,
      network_tx_kb_per_sec: 0.0,
    };
  };

  let delta_total = current.cpu_total.saturating_sub(prev.cpu_total);
  let delta_idle = current.cpu_idle.saturating_sub(prev.cpu_idle);
  let cpu_percent = if delta_total > 0 {
    (delta_total.saturating_sub(delta_idle) as f64 / delta_total as f64) * 100.0
  } else {
    0.0
  };

  let elapsed_secs = now.saturating_duration_since(prev.timestamp).as_secs_f64();
  let (network_rx_kb_per_sec, network_tx_kb_per_sec) = if elapsed_secs > 0.0 {
    (
      current.network_rx_bytes.saturating_sub(prev.network_rx_bytes) as f64 / 1024.0 / elapsed_secs,
      current.network_tx_bytes.saturating_sub(prev.network_tx_bytes) as f64 / 1024.0 / elapsed_secs,
    )
  } else {
    (0.0, 0.0)
  };

  SandboxResourceUsage { cpu_percent, memory_mb: current.memory_used_mb, network_rx_kb_per_sec, network_tx_kb_per_sec }
}

/// Samples a single sandbox's CPU/memory/network usage via one `sbx exec`
/// round trip (`RESOURCE_USAGE_SCRIPT`), computing CPU% and network KB/s
/// as deltas against `monitor`'s stored previous sample for `sandbox_id`
/// (see `compute_delta_usage`) — the same delta approach
/// `sample_host_stats` uses for host-wide stats, just per-sandbox and
/// keyed by id instead of a single shared `HostMonitor`.
///
/// Takes `&Mutex<SandboxMonitor>` rather than an already-locked guard so
/// the lock is only held for the synchronous delta computation, after the
/// `.await` below — holding a `std::sync::MutexGuard` across an `.await`
/// point would make this function's future non-`Send`, which Tauri's
/// async command dispatch requires.
///
/// Deliberately bypasses this module's shared `run()` helper, for the
/// same reason `read_branch_snapshot` does: a single failing command in
/// the `sh -c` chain (unlikely for these three `/proc` reads, but not
/// impossible if `/proc/net/dev` is briefly unavailable) shouldn't discard
/// whatever the earlier commands already printed. `parse_raw_proc_sample`
/// degrades missing sections to zeroed fields rather than erroring.
///
/// **UNVERIFIED**: no real `sbx` install is available in this dev
/// environment (see this module's top doc comment), so this exact
/// multi-command `sh -c` chaining and `/proc` output shape inside a real
/// sandbox VM is unconfirmed — the parsing logic itself is tested against
/// hand-built fixtures matching the documented kernel `/proc` format.
pub async fn sample_resource_usage<R: Runtime>(
  app: &AppHandle<R>,
  name: &str,
  sandbox_id: &str,
  monitor: &Mutex<SandboxMonitor>,
) -> Result<SandboxResourceUsage> {
  let output = app.shell().command("sbx").args(["exec", "-d", name, "sh", "-c", RESOURCE_USAGE_SCRIPT]).output().await?;
  let raw = parse_raw_proc_sample(&String::from_utf8_lossy(&output.stdout));
  let now = Instant::now();

  let mut monitor = monitor.lock().unwrap();
  let usage = compute_delta_usage(monitor.samples.get(sandbox_id), raw, now);
  monitor.samples.insert(
    sandbox_id.to_string(),
    SandboxSample {
      cpu_total: raw.cpu_total,
      cpu_idle: raw.cpu_idle,
      network_rx_bytes: raw.network_rx_bytes,
      network_tx_bytes: raw.network_tx_bytes,
      timestamp: now,
    },
  );
  Ok(usage)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reports_free_memory() {
    assert!(host_free_memory_mb() > 0.0);
  }

  #[test]
  fn classify_command_failure_recognizes_not_found_variants() {
    assert!(matches!(classify_command_failure("Error: sandbox 'foo' not found"), Some(Error::NotFound)));
    assert!(matches!(classify_command_failure("no such sandbox: foo"), Some(Error::NotFound)));
  }

  #[test]
  fn classify_command_failure_falls_through_for_unrecognized_stderr() {
    assert!(classify_command_failure("some other unexpected failure").is_none());
  }

  #[test]
  fn classify_command_failure_still_recognizes_existing_policy_and_container_errors() {
    assert!(matches!(classify_command_failure("network policy has not been initialized"), Some(Error::PolicyNotInitialized)));
    assert!(matches!(classify_command_failure("already initialized"), Some(Error::PolicyAlreadyInitialized)));
    assert!(matches!(classify_command_failure("failed to run sandbox container: boom"), Some(Error::ContainerStartFailed(_))));
  }

  #[test]
  fn parses_full_branch_snapshot() {
    let output = "main\n\
                   ---\n\
                   main\n\
                   feature\n\
                   ---\n\
                   worktree /home/agent/proj\n\
                   HEAD abc123\n\
                   branch refs/heads/main\n\
                   \n\
                   worktree /home/agent/proj-wt\n\
                   HEAD def456\n\
                   detached\n";
    let snapshot = parse_branch_snapshot(output);
    assert_eq!(
      snapshot,
      BranchSnapshot {
        current_branch: Some("main".to_string()),
        branches: vec!["main".to_string(), "feature".to_string()],
        worktrees: vec![
          WorktreeInfo { path: "/home/agent/proj".to_string(), branch: Some("main".to_string()), head_sha: "abc123".to_string() },
          WorktreeInfo { path: "/home/agent/proj-wt".to_string(), branch: None, head_sha: "def456".to_string() },
        ],
      }
    );
  }

  #[test]
  fn parses_detached_head_as_no_current_branch() {
    let snapshot = parse_branch_snapshot("HEAD\n---\nmain\n---\nworktree /home/agent/proj\nHEAD abc123\ndetached\n");
    assert_eq!(snapshot.current_branch, None);
  }

  #[test]
  fn parse_branch_snapshot_degrades_to_empty_on_non_git_output() {
    // A non-git workspace_path: every git subcommand fails, so `sh -c`
    // never gets far enough to print any of the `---` separators.
    let snapshot = parse_branch_snapshot("");
    assert_eq!(snapshot, BranchSnapshot::default());
  }

  #[test]
  fn parse_branch_snapshot_degrades_partial_output_from_a_mid_chain_failure() {
    // First command succeeded (branch printed), second's `---` separator
    // never printed because `for-each-ref` itself errored non-fatally
    // mid-script (still produces *a* `---` from the first echo, none after).
    let snapshot = parse_branch_snapshot("main\n---\n");
    assert_eq!(
      snapshot,
      BranchSnapshot { current_branch: Some("main".to_string()), branches: Vec::new(), worktrees: Vec::new() }
    );
  }

  #[test]
  fn shell_quote_escapes_embedded_single_quotes() {
    assert_eq!(shell_quote("/home/agent/proj"), "'/home/agent/proj'");
    assert_eq!(shell_quote("it's/here"), "'it'\\''s/here'");
  }

  #[test]
  fn container_start_failed_message_includes_raw_stderr_and_a_platform_hint() {
    let message = container_start_failed_message("ERROR: failed to run sandbox container");
    assert!(message.contains("ERROR: failed to run sandbox container"));
    #[cfg(target_os = "windows")]
    assert!(message.contains("HypervisorPlatform"));
  }

  #[test]
  fn parses_host_port_from_ports_table() {
    let output = "SANDBOX         AGENT   STATUS   PORTS                    WORKSPACE\nmy-sandbox      claude  running  127.0.0.1:8080->3000/tcp /home/user/proj";
    assert_eq!(parse_host_port(output, 3000), Some(8080));
    assert_eq!(parse_host_port(output, 9999), None);
  }

  #[test]
  fn parse_host_port_handles_empty_output() {
    assert_eq!(parse_host_port("", 3000), None);
  }

  #[test]
  fn parses_workspace_path_from_ls_table() {
    let output = "SANDBOX         AGENT   STATUS   PORTS                    WORKSPACE\nmy-sandbox      claude  running  127.0.0.1:8080->3000/tcp /home/user/proj";
    assert_eq!(parse_workspace_path(output, "my-sandbox"), Some("/home/user/proj".to_string()));
    assert_eq!(parse_workspace_path(output, "other-sandbox"), None);
  }

  #[test]
  fn parses_workspace_path_with_no_ports_column() {
    let output = "SANDBOX     AGENT    STATUS    PORTS   WORKSPACE\nmy-sandbox  claude   running           ~/my-project";
    assert_eq!(parse_workspace_path(output, "my-sandbox"), Some("~/my-project".to_string()));
  }

  #[test]
  fn parse_workspace_path_handles_empty_output() {
    assert_eq!(parse_workspace_path("", "my-sandbox"), None);
  }

  #[test]
  fn parses_sandbox_status_from_ls_table() {
    let output = "SANDBOX         AGENT   STATUS   PORTS                    WORKSPACE\nmy-sandbox      claude  running  127.0.0.1:8080->3000/tcp /home/user/proj";
    assert_eq!(parse_sandbox_status(output, "my-sandbox"), Some("running".to_string()));
    assert_eq!(parse_sandbox_status(output, "other-sandbox"), None);
  }

  #[test]
  fn parse_sandbox_status_handles_empty_output() {
    assert_eq!(parse_sandbox_status("", "my-sandbox"), None);
  }

  #[test]
  fn parses_every_row_from_ls_table() {
    let output = "SANDBOX         AGENT   STATUS   PORTS                    WORKSPACE\n\
                   my-sandbox      claude  running  127.0.0.1:8080->3000/tcp /home/user/proj\n\
                   other-sandbox   claude  stopped                          ~/other-project";
    let rows = parse_sandbox_rows(output);
    assert_eq!(
      rows,
      vec![
        SbxListRow {
          sbx_name: "my-sandbox".to_string(),
          status: "running".to_string(),
          workspace_path: Some("/home/user/proj".to_string()),
        },
        SbxListRow {
          sbx_name: "other-sandbox".to_string(),
          status: "stopped".to_string(),
          workspace_path: Some("~/other-project".to_string()),
        },
      ]
    );
  }

  #[test]
  fn parse_sandbox_rows_skips_rows_with_no_workspace_column() {
    let output = "SANDBOX      STATUS\nmy-sandbox   running";
    // Only 2 tokens (no AGENT column here either) — degrades to nothing
    // rather than misreading STATUS as the name/workspace.
    assert_eq!(parse_sandbox_rows(output), Vec::new());
  }

  #[test]
  fn parse_sandbox_rows_handles_empty_output() {
    assert_eq!(parse_sandbox_rows(""), Vec::new());
  }

  #[test]
  fn parse_sandbox_rows_returns_empty_when_no_header_row_is_present() {
    let output = "some   garbage   line\nanother   line   here";
    assert_eq!(parse_sandbox_rows(output), Vec::new());
  }

  #[test]
  fn parse_sandbox_rows_skips_leading_blank_lines_before_header() {
    let output = "\n\nSANDBOX AGENT STATUS WORKSPACE\nfoo claude running /repo\n";
    assert_eq!(parse_sandbox_rows(output).len(), 1);
  }

  #[test]
  fn parse_sandbox_rows_ignores_the_no_sandboxes_empty_state_box() {
    // `sbx ls` with nothing running prints a bordered empty-state message,
    // not a table — reproduces the user-reported bug where every line of
    // this box (>=3 whitespace tokens, first token != "SANDBOX") was
    // misread as a data row, fabricating 2-3 orphan sandboxes per poll.
    let output = "\
      \u{2502}  No Sandboxes found.        \u{2502}\n\
      \u{2502}  Launch one: sbx run claude \u{2502}";
    assert_eq!(parse_sandbox_rows(output), Vec::new());
  }

  #[test]
  fn parse_sandbox_rows_ignores_the_empty_state_box_even_when_header_is_still_printed() {
    // Real `sbx ls` prints the "SANDBOX ..." header unconditionally, even
    // with zero sandboxes, and only swaps the *body* for the decorated
    // empty-state box below it. The header-presence guard added to fix the
    // no-header case doesn't help here: `saw_header` becomes true, and the
    // box's content line then satisfies the old ">=3 tokens" heuristic,
    // fabricating a fake row (sbx_name="│", status="Sandboxes") — matching
    // a real corrupted DB row: id=..., project_id=unassigned, mode=clone,
    // status=Sandboxes, sbx_name=│.
    let output = "\
      SANDBOX         AGENT   STATUS   PORTS                    WORKSPACE\n\
      \u{2502}  No Sandboxes found.        \u{2502}\n\
      \u{2502}  Launch one: sbx run claude \u{2502}";
    assert_eq!(parse_sandbox_rows(output), Vec::new());
  }

  #[test]
  fn list_all_normalizes_workspace_paths() {
    let output = "SANDBOX         AGENT   STATUS   PORTS  WORKSPACE\n\
                   win-sandbox     claude  running         F:\\works\\proj\n\
                   tilde-sandbox   claude  running         ~/my-project";
    let rows = parse_sandbox_rows(output)
      .into_iter()
      .map(|row| SbxListRow {
        workspace_path: row.workspace_path.map(|path| expand_home(&windows_path_to_posix(&path))),
        ..row
      })
      .collect::<Vec<_>>();
    assert_eq!(rows[0].workspace_path.as_deref(), Some("/f/works/proj"));
    assert_eq!(rows[1].workspace_path.as_deref(), Some("/home/agent/my-project"));
  }

  #[test]
  fn expands_home_shorthand() {
    assert_eq!(expand_home("~/my-project"), "/home/agent/my-project");
    assert_eq!(expand_home("~"), "/home/agent");
    assert_eq!(expand_home("/home/user/proj"), "/home/user/proj");
  }

  #[test]
  fn converts_windows_path_to_posix() {
    assert_eq!(windows_path_to_posix(r"F:\works\personal\remind-me"), "/f/works/personal/remind-me");
    assert_eq!(windows_path_to_posix(r"C:\Users\showr"), "/c/Users/showr");
  }

  #[test]
  fn leaves_non_windows_paths_alone() {
    assert_eq!(windows_path_to_posix("/home/user/project"), "/home/user/project");
    assert_eq!(windows_path_to_posix("~/my-project"), "~/my-project");
  }

  #[test]
  fn builds_cp_from_sandbox_args() {
    assert_eq!(
      cp_from_sandbox_args("my-sandbox", "/home/agent/.claude", "/host/dest"),
      vec!["cp", "my-sandbox:/home/agent/.claude", "/host/dest"]
    );
  }

  #[test]
  fn builds_cp_to_sandbox_args() {
    assert_eq!(
      cp_to_sandbox_args("my-sandbox", "/host/backup/claude", "/home/agent/.claude"),
      vec!["cp", "/host/backup/claude", "my-sandbox:/home/agent/.claude"]
    );
  }

  #[test]
  fn restore_directory_script_quotes_both_paths() {
    let script = restore_directory_script("/tmp/overnight-restore-abc", "/home/agent/.claude", &[]);
    assert!(script.contains("src='/tmp/overnight-restore-abc'"));
    assert!(script.contains("mkdir -p '/home/agent/.claude'"));
    assert!(script.contains("rm -rf '/home/agent/.claude'/\"$name\" || fail=1"));
    assert!(script.contains("mv \"$entry\" '/home/agent/.claude'/\"$name\" 2>/dev/null || cp -a \"$entry\" '/home/agent/.claude'/\"$name\" || fail=1"));
    assert!(script.contains("rm -rf '/tmp/overnight-restore-abc' 2>/dev/null"));
  }

  #[test]
  fn restore_directory_script_escapes_embedded_quotes() {
    let script = restore_directory_script("/tmp/it's", "/dest/it's", &[]);
    assert!(script.contains("'/tmp/it'\\''s'"));
    assert!(script.contains("'/dest/it'\\''s'"));
  }

  #[test]
  fn restore_directory_script_clears_then_refills_named_entries() {
    let script = restore_directory_script("/tmp/scratch", "/home/agent/.claude", CLAUDE_HOME_MOUNTED_DIRS);
    assert!(script.contains("case \"$name\" in projects|sessions|shell-snapshots|statsig|todos|skills)"));
    // Clears existing contents before refilling — not a plain additive merge.
    // The clear itself stays best-effort (a stray root-owned lost+found can
    // survive it harmlessly); the actual copy-in must not be swallowed.
    assert!(script.contains("rm -rf '/home/agent/.claude'/\"$name\"/.[!.]* '/home/agent/.claude'/\"$name\"/..?* '/home/agent/.claude'/\"$name\"/* 2>/dev/null"));
    assert!(script.contains("cp -a \"$entry\"/. '/home/agent/.claude'/\"$name\"/ || fail=1"));
    // Falls through to the normal replace path for everything else.
    assert!(script.contains("rm -rf '/home/agent/.claude'/\"$name\" || fail=1;"));
    assert!(script.contains("mv \"$entry\" '/home/agent/.claude'/\"$name\" 2>/dev/null || cp -a \"$entry\" '/home/agent/.claude'/\"$name\" || fail=1"));
  }

  #[test]
  fn restore_directory_script_propagates_failure_instead_of_always_exiting_zero() {
    let script = restore_directory_script("/tmp/scratch", "/home/agent/.claude", CLAUDE_HOME_MOUNTED_DIRS);
    // A script that ends on a bare `true` masks every per-entry mv/cp
    // failure that happened earlier — the restore would report success
    // even if a file never made it across.
    assert!(!script.trim_end().ends_with("true"));
    assert!(script.contains("fail=0"));
    assert!(script.trim_end().ends_with("exit $fail"));
  }

  #[test]
  fn restore_directory_script_has_no_case_guard_when_nothing_merged() {
    let script = restore_directory_script("/tmp/scratch", "/home/agent/.claude", &[]);
    assert!(!script.contains("case \"$name\" in"));
  }

  #[test]
  fn builds_git_config_exec_args() {
    assert_eq!(
      git_config_exec_args("my-sandbox", "user.name", "Jane Doe"),
      vec!["exec", "-d", "my-sandbox", "git", "config", "--global", "user.name", "Jane Doe"]
    );
  }

  #[test]
  fn parse_policy_rules_handles_empty_output() {
    assert_eq!(parse_policy_rules(""), Vec::new());
  }

  #[test]
  fn parse_policy_rules_degrades_to_empty_on_unrecognized_header() {
    let output = "SOMETHING ELSE\nunrelated line";
    assert_eq!(parse_policy_rules(output), Vec::new());
  }

  // Verbatim excerpt (trimmed) of real `sbx policy ls <name> --wide`
  // output reported against a live Windows install — a multi-host network
  // rule wrapping onto continuation lines, then two filesystem rules that
  // must be filtered out.
  const REAL_WIDE_OUTPUT: &str = "\
SOURCE   APPLIES TO   POLICY   POLICY_ID      RULE   RULE_ID                        TYPE               DECISION   RESOURCES
local    all          -        local-policy   -      default-ai-services            network            allow      **.chatgpt.com:443
                                                                                                                  **.cursor.sh:443
                                                                                                                  api.anthropic.com:443

local    all          -        local-policy   -      default-fs-read-allow-all      filesystem:read    allow      **

local    all          -        local-policy   -      default-fs-write-allow-all     filesystem:write   allow      **
";

  #[test]
  fn parse_policy_rules_expands_wrapped_resources_and_filters_by_type() {
    let rules = parse_policy_rules(REAL_WIDE_OUTPUT);
    assert_eq!(
      rules,
      vec![
        PolicyRule { host: "**.chatgpt.com:443".into(), decision: "allow".into(), source: "local".into() },
        PolicyRule { host: "**.cursor.sh:443".into(), decision: "allow".into(), source: "local".into() },
        PolicyRule { host: "api.anthropic.com:443".into(), decision: "allow".into(), source: "local".into() },
      ]
    );
  }

  #[test]
  fn parses_cpu_jiffies_from_proc_stat() {
    // Real /proc/stat shape: an aggregate `cpu` line, then per-core
    // `cpu0`/`cpu1`/... lines that must be ignored.
    let stat = "cpu  10132153 290696 3084719 46828483 16683 0 25195 0 175628 0\n\
                cpu0 1234 0 0 5678 0 0 0 0 0 0\n\
                intr 123456 0 0 0\n";
    let (total, idle) = parse_cpu_jiffies(stat).unwrap();
    // idle (46828483) + iowait (16683)
    assert_eq!(idle, 46828483 + 16683);
    assert_eq!(total, 10132153 + 290696 + 3084719 + 46828483 + 16683 + 25195 + 175628);
  }

  #[test]
  fn parse_cpu_jiffies_returns_none_when_no_aggregate_line() {
    assert_eq!(parse_cpu_jiffies(""), None);
    assert_eq!(parse_cpu_jiffies("cpu0 1234 0 0 5678\n"), None);
  }

  #[test]
  fn parses_memory_used_mb_with_mem_available() {
    let meminfo = "MemTotal:       16384000 kB\n\
                   MemFree:         1024000 kB\n\
                   MemAvailable:    2048000 kB\n\
                   Buffers:          100000 kB\n";
    // (16384000 - 2048000) kB -> MB
    assert_eq!(parse_memory_used_mb(meminfo), (16384000.0 - 2048000.0) / 1024.0);
  }

  #[test]
  fn parses_memory_used_mb_falls_back_to_mem_free_without_mem_available() {
    let meminfo = "MemTotal:       16384000 kB\nMemFree:         1024000 kB\n";
    assert_eq!(parse_memory_used_mb(meminfo), (16384000.0 - 1024000.0) / 1024.0);
  }

  #[test]
  fn parse_memory_used_mb_handles_empty_output() {
    assert_eq!(parse_memory_used_mb(""), 0.0);
  }

  #[test]
  fn parses_network_bytes_summing_non_loopback_interfaces() {
    let net_dev = "Inter-|   Receive                                                |  Transmit\n\
                    face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n\
                       lo:  1296      16    0    0    0     0          0         0     1296      16    0    0    0     0       0          0\n\
                     eth0: 1000000     100    0    0    0     0          0         0  2000000     200    0    0    0     0       0          0\n\
                     eth1:  500000      50    0    0    0     0          0         0   250000      25    0    0    0     0       0          0\n";
    let (rx, tx) = parse_network_bytes(net_dev);
    // lo is excluded; eth0 + eth1 summed.
    assert_eq!(rx, 1000000 + 500000);
    assert_eq!(tx, 2000000 + 250000);
  }

  #[test]
  fn parse_network_bytes_handles_empty_output() {
    assert_eq!(parse_network_bytes(""), (0, 0));
  }

  #[test]
  fn parses_full_raw_proc_sample_from_combined_script_output() {
    let output = "cpu  100 0 100 800 0 0 0 0 0 0\n\
                  ---\n\
                  MemTotal:       1000000 kB\n\
                  MemAvailable:    400000 kB\n\
                  ---\n\
                  Inter-|   Receive                                                |  Transmit\n\
                   face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n\
                      lo:     0       0    0    0    0     0          0         0        0       0    0    0    0     0       0          0\n\
                    eth0: 12345     10    0    0    0     0          0         0    54321      20    0    0    0     0       0          0\n";
    let sample = parse_raw_proc_sample(output);
    assert_eq!(
      sample,
      RawProcSample {
        cpu_total: 100 + 100 + 800,
        cpu_idle: 800,
        memory_used_mb: (1000000.0 - 400000.0) / 1024.0,
        network_rx_bytes: 12345,
        network_tx_bytes: 54321,
      }
    );
  }

  #[test]
  fn parse_raw_proc_sample_degrades_to_zeros_on_empty_output() {
    assert_eq!(parse_raw_proc_sample(""), RawProcSample::default());
  }

  #[test]
  fn compute_delta_usage_returns_zeros_for_cpu_and_network_on_first_sample() {
    let current = RawProcSample { cpu_total: 1000, cpu_idle: 800, memory_used_mb: 512.0, network_rx_bytes: 5000, network_tx_bytes: 2000 };
    let usage = compute_delta_usage(None, current, Instant::now());
    assert_eq!(
      usage,
      SandboxResourceUsage { cpu_percent: 0.0, memory_mb: 512.0, network_rx_kb_per_sec: 0.0, network_tx_kb_per_sec: 0.0 }
    );
  }

  #[test]
  fn compute_delta_usage_computes_cpu_percent_and_network_rate_from_previous_sample() {
    let t0 = Instant::now();
    let prev = SandboxSample { cpu_total: 1000, cpu_idle: 800, network_rx_bytes: 10_000, network_tx_bytes: 4_000, timestamp: t0 };
    let current = RawProcSample { cpu_total: 1500, cpu_idle: 900, memory_used_mb: 550.0, network_rx_bytes: 20_240, network_tx_bytes: 14_240 };
    let t1 = t0 + std::time::Duration::from_secs(2);

    let usage = compute_delta_usage(Some(&prev), current, t1);

    // delta_total = 500, delta_idle = 100 -> busy = 400/500 = 80%
    assert_eq!(usage.cpu_percent, 80.0);
    assert_eq!(usage.memory_mb, 550.0);
    // delta_rx = 10240 bytes = 10 KB over 2s -> 5 KB/s
    assert_eq!(usage.network_rx_kb_per_sec, 5.0);
    // delta_tx = 10240 bytes = 10 KB over 2s -> 5 KB/s
    assert_eq!(usage.network_tx_kb_per_sec, 5.0);
  }

  #[test]
  fn compute_delta_usage_guards_against_counters_going_backwards() {
    // A sandbox restart between polls resets /proc's counters, so the
    // "current" reading can be lower than the stored "previous" one —
    // saturating_sub must prevent an underflowed delta.
    let t0 = Instant::now();
    let prev = SandboxSample { cpu_total: 5000, cpu_idle: 4000, network_rx_bytes: 50_000, network_tx_bytes: 50_000, timestamp: t0 };
    let current = RawProcSample { cpu_total: 100, cpu_idle: 80, memory_used_mb: 200.0, network_rx_bytes: 100, network_tx_bytes: 100 };
    let t1 = t0 + std::time::Duration::from_secs(1);

    let usage = compute_delta_usage(Some(&prev), current, t1);
    assert_eq!(usage.cpu_percent, 0.0);
    assert_eq!(usage.network_rx_kb_per_sec, 0.0);
    assert_eq!(usage.network_tx_kb_per_sec, 0.0);
    assert_eq!(usage.memory_mb, 200.0);
  }

  #[test]
  fn compute_delta_usage_handles_zero_elapsed_time() {
    let t0 = Instant::now();
    let prev = SandboxSample { cpu_total: 1000, cpu_idle: 800, network_rx_bytes: 10_000, network_tx_bytes: 4_000, timestamp: t0 };
    let current = RawProcSample { cpu_total: 1500, cpu_idle: 900, memory_used_mb: 550.0, network_rx_bytes: 20_000, network_tx_bytes: 14_000 };

    let usage = compute_delta_usage(Some(&prev), current, t0);
    assert_eq!(usage.network_rx_kb_per_sec, 0.0);
    assert_eq!(usage.network_tx_kb_per_sec, 0.0);
  }
}

#[cfg(test)]
mod env_persist_tests {
  use super::*;
  use std::process::Command;

  fn temp_persistent_file() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("overnight-env-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("persistent.sh");
    std::fs::write(&path, "").unwrap();
    path
  }

  fn apply(vars: &[EnvVar], path: &std::path::Path) -> String {
    let script = build_env_persist_script(vars, path.to_str().unwrap());
    let status = Command::new("bash").arg("-c").arg(&script).status().unwrap();
    assert!(status.success());
    std::fs::read_to_string(path).unwrap()
  }

  fn sourced_value(contents: &str, key: &str) -> String {
    let script = format!("{contents}\nprintf '%s' \"${key}\"");
    let output = Command::new("bash").arg("-c").arg(&script).output().unwrap();
    String::from_utf8(output.stdout).unwrap()
  }

  #[test]
  fn writes_and_sources_a_value_with_quotes_and_spaces() {
    let path = temp_persistent_file();
    let vars = vec![EnvVar { key: "TOKEN".to_string(), value: "can't \"stop\" me".to_string() }];
    let contents = apply(&vars, &path);
    assert_eq!(sourced_value(&contents, "TOKEN"), "can't \"stop\" me");
  }

  #[test]
  fn re_running_replaces_rather_than_duplicates() {
    let path = temp_persistent_file();
    apply(&[EnvVar { key: "A".to_string(), value: "1".to_string() }], &path);
    let contents = apply(&[EnvVar { key: "A".to_string(), value: "2".to_string() }], &path);
    assert_eq!(contents.matches("export A=").count(), 1);
    assert_eq!(sourced_value(&contents, "A"), "2");
  }

  #[test]
  fn removing_a_var_clears_it_on_next_apply() {
    let path = temp_persistent_file();
    apply(&[EnvVar { key: "A".to_string(), value: "1".to_string() }], &path);
    let contents = apply(&[], &path);
    assert!(!contents.contains("export A="));
  }

  #[test]
  fn preserves_existing_file_content_outside_the_managed_block() {
    let path = temp_persistent_file();
    std::fs::write(&path, "export EXISTING=kept\n").unwrap();
    let contents = apply(&[EnvVar { key: "NEW".to_string(), value: "1".to_string() }], &path);
    assert!(contents.contains("export EXISTING=kept"));
    assert!(contents.contains("export NEW='1'"));
  }

  // Reproduces a real failure seen against an actual sbx sandbox: the
  // previous sed -i based implementation needs write access to the
  // *containing directory* (to create its own temp file before renaming),
  // which /etc isn't, even though /etc/sandbox-persistent.sh itself is.
  #[cfg(unix)]
  #[test]
  fn works_when_the_containing_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("overnight-env-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("persistent.sh");
    std::fs::write(&path, "export EXISTING=kept\n").unwrap();

    let mut perms = std::fs::metadata(&dir).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&dir, perms).unwrap();

    let contents = apply(&[EnvVar { key: "NEW".to_string(), value: "1".to_string() }], &path);

    // Restore write permission so the temp dir can be cleaned up normally.
    let mut restored = std::fs::metadata(&dir).unwrap().permissions();
    restored.set_mode(0o755);
    std::fs::set_permissions(&dir, restored).unwrap();

    assert!(contents.contains("export EXISTING=kept"));
    assert!(contents.contains("export NEW='1'"));
  }

  #[test]
  fn redact_env_vars_keeps_keys_but_masks_values() {
    let vars = vec![EnvVar { key: "TOKEN".to_string(), value: "super-secret".to_string() }];
    let redacted = redact_env_vars(&vars);
    assert_eq!(redacted[0].key, "TOKEN");
    assert_eq!(redacted[0].value, "********");

    let script = build_env_persist_script(&redacted, "/etc/sandbox-persistent.sh");
    assert!(!script.contains("super-secret"));
    assert!(script.contains("TOKEN"));
    assert!(script.contains("********"));
  }

  #[test]
  fn service_secret_set_args_value_source() {
    let source = SecretSource::Value { value: "sk-abc".to_string() };
    assert_eq!(service_secret_set_args("anthropic", &source, None), vec!["secret", "set", "anthropic", "--value", "sk-abc"]);
  }

  #[test]
  fn service_secret_set_args_sandbox_scoped_reference_source_with_refresh() {
    let source = SecretSource::Reference {
      reference: "op://Work/Anthropic/credential".to_string(),
      refresh: Some("30m".to_string()),
    };
    assert_eq!(
      service_secret_set_args("anthropic", &source, Some("my-sbx")),
      vec![
        "secret", "set", "anthropic", "--sandbox", "my-sbx", "--show-error", "--ref", "op://Work/Anthropic/credential", "--refresh",
        "30m"
      ]
    );
  }

  #[test]
  fn service_secret_set_args_command_source_no_refresh() {
    let source = SecretSource::Command { command: "gh auth token".to_string(), refresh: None };
    assert_eq!(
      service_secret_set_args("github", &source, None),
      vec!["secret", "set", "github", "--show-error", "--command", "gh auth token"]
    );
  }

  #[test]
  fn service_secret_rm_args() {
    assert_eq!(super::service_secret_rm_args("github", None), vec!["secret", "rm", "github", "-f"]);
    assert_eq!(
      super::service_secret_rm_args("github", Some("my-sbx")),
      vec!["secret", "rm", "github", "--sandbox", "my-sbx", "-f"]
    );
  }

  #[test]
  fn custom_secret_set_args_value_source_multiple_hosts() {
    let source = SecretSource::Value { value: "tok".to_string() };
    assert_eq!(
      custom_secret_set_args(
        "API_KEY",
        &["api.example.com".to_string(), "uploads.example.com".to_string()],
        "sbx-cs-fixed123",
        &source,
        None
      ),
      vec![
        "secret",
        "set-custom",
        "--host",
        "api.example.com",
        "--host",
        "uploads.example.com",
        "--env",
        "API_KEY",
        "--placeholder",
        "sbx-cs-fixed123",
        "--value",
        "tok"
      ]
    );
  }

  #[test]
  fn custom_secret_set_args_never_adds_show_error_for_a_literal_value() {
    // Regression: `sbx` rejects --show-error unless --ref or --command is
    // also present ("ERROR: --show-error requires --ref or --command").
    let source = SecretSource::Value { value: "tok".to_string() };
    let args = custom_secret_set_args("API_KEY", &["a.com".to_string()], "sbx-cs-x", &source, None);
    assert!(!args.contains(&"--show-error".to_string()));
  }

  #[test]
  fn custom_secret_set_args_sandbox_scoped_command_source_with_refresh() {
    let source = SecretSource::Command { command: "print-secret".to_string(), refresh: Some("on-demand".to_string()) };
    assert_eq!(
      custom_secret_set_args("API_KEY", &["api.example.com".to_string()], "sbx-cs-fixed123", &source, Some("my-sbx")),
      vec![
        "secret",
        "set-custom",
        "--sandbox",
        "my-sbx",
        "--host",
        "api.example.com",
        "--env",
        "API_KEY",
        "--placeholder",
        "sbx-cs-fixed123",
        "--show-error",
        "--command",
        "print-secret",
        "--refresh",
        "on-demand"
      ]
    );
  }

  #[test]
  fn custom_secret_rm_args() {
    assert_eq!(super::custom_secret_rm_args("api.example.com", None), vec!["secret", "rm", "--host", "api.example.com", "-f"]);
    assert_eq!(
      super::custom_secret_rm_args("api.example.com", Some("my-sbx")),
      vec!["secret", "rm", "--sandbox", "my-sbx", "--host", "api.example.com", "-f"]
    );
  }

  #[test]
  fn redact_value_arg_masks_the_value_but_not_other_flags() {
    let args = vec!["secret".to_string(), "set".to_string(), "anthropic".to_string(), "--value".to_string(), "sk-abc".to_string()];
    let redacted = redact_value_arg(&args);
    assert_eq!(redacted, vec!["secret", "set", "anthropic", "--value", "********"]);
  }

  #[test]
  fn redact_value_arg_leaves_a_reference_or_command_source_untouched() {
    let args = vec!["secret".to_string(), "set".to_string(), "anthropic".to_string(), "--ref".to_string(), "op://Work/x".to_string()];
    assert_eq!(redact_value_arg(&args), args);
  }
}



