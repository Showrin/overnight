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

use tauri::{AppHandle, Runtime};
use tauri_plugin_shell::ShellExt;

use crate::db::models::WorktreeInfo;
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
}

pub type Result<T> = std::result::Result<T, Error>;

async fn run<R: Runtime>(app: &AppHandle<R>, args: &[&str]) -> Result<String> {
  let output = app.shell().command("sbx").args(args).output().await?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("network policy has not been initialized") {
      return Err(Error::PolicyNotInitialized);
    }
    if stderr.contains("already initialized") {
      return Err(Error::PolicyAlreadyInitialized);
    }
    if stderr.contains("failed to run sandbox container") {
      return Err(Error::ContainerStartFailed(container_start_failed_message(&stderr)));
    }
    return Err(Error::CommandFailed(format!(
      "sbx {args:?} exited with {:?}: {stderr}",
      output.status.code(),
    )));
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
  run(app, &["setup", "ssh"]).await?;
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
  run(app, &["policy", "init", preset]).await?;
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
  run(app, &["policy", "reset", "--force"]).await?;
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
  run(app, &args).await?;
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
  run(app, &args).await?;
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
  let output = run(app, &args).await?;
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
  run(app, &["secret", "set", "anthropic", "-t", token]).await?;
  Ok(())
}

/// Health check for the Sandboxes page. `sbx ls` is read-only, so (unlike
/// `sbx run`/`sbx create`) it shouldn't trigger the interactive first-run
/// network-policy prompt — it errors cleanly if `sbx` isn't installed or
/// isn't logged in.
pub async fn health_check<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
  run(app, &["ls"]).await?;
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
  run(app, &args).await?;
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
  run(app, &args.iter().map(String::as_str).collect::<Vec<_>>()).await?;
  Ok(())
}

fn git_config_exec_args(name: &str, key: &str, value: &str) -> Vec<String> {
  ["exec", "-d", name, "git", "config", "--global", key, value].map(String::from).to_vec()
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
  let output = app.shell().command("sbx").args(["exec", "-d", name, "sh", "-c", &script]).output().await?;
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
  run(app, &["stop", name]).await?;
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
  crate::process::spawn(app, "sbx", &args, None)?;
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
/// **UNVERIFIED**: no real `sbx` install is available in this dev
/// environment (see this module's top doc comment), so the exact `sbx cp`
/// argument syntax for a directory source, and whether it creates
/// `host_dest` itself or requires it to already exist, are unconfirmed —
/// callers should keep pre-creating the destination directory (as
/// `backup_sandbox_claude_data` does) until this has been exercised against
/// a real sandbox.
pub async fn cp_from_sandbox<R: Runtime>(app: &AppHandle<R>, name: &str, remote_path: &str, host_dest: &str) -> Result<()> {
  let args = cp_from_sandbox_args(name, remote_path, host_dest);
  run(app, &args.iter().map(String::as_str).collect::<Vec<_>>()).await?;
  Ok(())
}

fn cp_from_sandbox_args(name: &str, remote_path: &str, host_dest: &str) -> Vec<String> {
  vec!["cp".to_string(), format!("{name}:{remote_path}"), host_dest.to_string()]
}

/// Force-removes the sandbox and its VM (used by explicit sandbox Delete).
pub async fn rm<R: Runtime>(app: &AppHandle<R>, name: &str) -> Result<()> {
  run(app, &["rm", "--force", name]).await?;
  Ok(())
}

/// Publishes `sandbox_port` on an OS-assigned host port. Use `host_port`
/// afterward to find out which port was assigned.
pub async fn publish_port<R: Runtime>(app: &AppHandle<R>, name: &str, sandbox_port: u16) -> Result<()> {
  run(app, &["ports", name, "--publish", &sandbox_port.to_string()]).await?;
  Ok(())
}

/// Parses `sbx ports <name>` output for the host port mapped to
/// `sandbox_port`. Expected line shape (per docs):
/// `127.0.0.1:8080->3000/tcp` — no confirmed `--format json`, so this is a
/// best-effort regex-free parse of that pattern.
pub async fn host_port<R: Runtime>(app: &AppHandle<R>, name: &str, sandbox_port: u16) -> Result<Option<u16>> {
  let output = run(app, &["ports", name]).await?;
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
  let output = run(app, &["ls"]).await?;
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
  let output = run(app, &["ls"]).await?;
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

/// Parses every non-header row of `sbx ls` output using the same
/// fixed-position layout `parse_sandbox_status`/`parse_workspace_path`
/// already assume (`SANDBOX AGENT STATUS [PORTS] WORKSPACE`): first token
/// is the name, third is status, last is the workspace path — but only
/// when there are more than 3 tokens, so a row with no workspace column
/// doesn't misread its own status token as a path.
fn parse_sandbox_rows(output: &str) -> Vec<SbxListRow> {
  output
    .lines()
    .filter(|line| !line.trim().is_empty())
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
  let output = run(app, &["ls"]).await?;
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
    let output = run(app, &["ls"]).await?;
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
  Ok(crate::process::spawn(app, "sbx", &args, None)?)
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
