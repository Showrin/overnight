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
}

pub type Result<T> = std::result::Result<T, Error>;

async fn run<R: Runtime>(app: &AppHandle<R>, args: &[&str]) -> Result<String> {
  let output = app.shell().command("sbx").args(args).output().await?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("network policy has not been initialized") {
      return Err(Error::PolicyNotInitialized);
    }
    return Err(Error::CommandFailed(format!(
      "sbx {args:?} exited with {:?}: {stderr}",
      output.status.code(),
    )));
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reports_free_memory() {
    assert!(host_free_memory_mb() > 0.0);
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
}
