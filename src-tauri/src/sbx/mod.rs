//! Thin wrapper around the `sbx` CLI (Docker Sandboxes — microVM-isolated
//! agent sandboxes), built on `process::spawn`/`tauri_plugin_shell`. No
//! caller outside this module should shell out to `sbx` directly.
//!
//! Several things here are best-effort readings of https://docs.docker.com/ai/sandboxes/
//! rather than verified against a real `sbx` install (none is available in
//! this dev environment) — see the doc comment on each function that says so.

use tauri::{AppHandle, Runtime};
use tauri_plugin_shell::ShellExt;

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

/// `sbx policy init <preset>` — one-time, machine-wide setup that answers
/// the interactive network-policy prompt headlessly. `preset` must be one
/// of `allow-all`, `balanced`, or `deny-all` (sbx's own accepted values).
pub async fn policy_init<R: Runtime>(app: &AppHandle<R>, preset: &str) -> Result<()> {
  run(app, &["policy", "init", preset]).await?;
  Ok(())
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

/// `sbx cp <local> <name>:<remote>` — copies a single file or directory
/// from the host into a running sandbox's filesystem (directory copies are
/// recursive per the docs). Used to bring `extra_clone_paths` — files git
/// ignores, like `.env`, that a clone-mode `sbx create --clone` therefore
/// doesn't carry over — into the sandbox after it's created. Nested file
/// destinations need their parent directory to exist first; see `mkdir`.
pub async fn cp<R: Runtime>(app: &AppHandle<R>, local_path: &str, name: &str, remote_path: &str) -> Result<()> {
  run(app, &["cp", local_path, &format!("{name}:{remote_path}")]).await?;
  Ok(())
}

/// `sbx exec -d <name> mkdir -p <path>` — ensures a directory exists inside
/// the sandbox before `cp`-ing a file into it.
pub async fn mkdir<R: Runtime>(app: &AppHandle<R>, name: &str, path: &str) -> Result<()> {
  run(app, &["exec", "-d", name, "mkdir", "-p", path]).await?;
  Ok(())
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
fn windows_path_to_posix(path: &str) -> String {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reports_free_memory() {
    assert!(host_free_memory_mb() > 0.0);
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
}
