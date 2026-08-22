//! Thin wrapper around the `docker` and `git` CLIs, built on
//! `process::spawn`/`tauri_plugin_shell`. No caller outside this module
//! should shell out to `docker`/`git` directly — this is the single place
//! that knows the actual command-line shape.

use std::net::TcpListener;
use std::path::Path;

use serde::Deserialize;
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
  #[error("failed to parse docker output: {0}")]
  ParseFailed(String),
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

async fn run<R: Runtime>(app: &AppHandle<R>, program: &str, args: &[&str]) -> Result<String> {
  let output = app.shell().command(program).args(args).output().await?;
  if !output.status.success() {
    return Err(Error::CommandFailed(format!(
      "{program} {args:?} exited with {:?}: {}",
      output.status.code(),
      String::from_utf8_lossy(&output.stderr).trim()
    )));
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Health check for the Sandboxes page — errors if the Docker daemon isn't
/// reachable (not installed, not running, permission denied, etc).
pub async fn info<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
  run(app, "docker", &["info"]).await?;
  Ok(())
}

pub struct RunOptions<'a> {
  pub image: &'a str,
  pub name: &'a str,
  /// (host path, container path) — bind mount for the project/sandbox folder.
  pub mount: (&'a str, &'a str),
  pub host_port: u16,
  pub container_port: u16,
  pub memory_mb: u32,
}

/// `docker run -d ...`, returns the new container's id. The container just
/// sleeps — agent sessions and terminals attach into it via `exec`.
pub async fn run_container<R: Runtime>(app: &AppHandle<R>, opts: &RunOptions<'_>) -> Result<String> {
  let mount = format!("{}:{}", opts.mount.0, opts.mount.1);
  let port = format!("{}:{}", opts.host_port, opts.container_port);
  let memory = format!("{}m", opts.memory_mb);
  run(
    app,
    "docker",
    &[
      "run",
      "-d",
      "--name",
      opts.name,
      "--memory",
      &memory,
      "-p",
      &port,
      "-v",
      &mount,
      "-w",
      opts.mount.1,
      opts.image,
      "sleep",
      "infinity",
    ],
  )
  .await
}

pub async fn stop<R: Runtime>(app: &AppHandle<R>, container_id: &str) -> Result<()> {
  run(app, "docker", &["stop", container_id]).await?;
  Ok(())
}

pub async fn start<R: Runtime>(app: &AppHandle<R>, container_id: &str) -> Result<()> {
  run(app, "docker", &["start", container_id]).await?;
  Ok(())
}

/// Force-removes the container (used by explicit sandbox Delete).
pub async fn rm<R: Runtime>(app: &AppHandle<R>, container_id: &str) -> Result<()> {
  run(app, "docker", &["rm", "-f", container_id]).await?;
  Ok(())
}

// exec/logs_stream/stats and the parse_* helpers below aren't called by any
// command yet — they're wired up by the metrics-polling/logs/terminal step
// of the Sandboxes page, not the lifecycle (create/start/stop/delete) step.
#[allow(dead_code)]
pub async fn exec<R: Runtime>(app: &AppHandle<R>, container_id: &str, cmd: &[&str]) -> Result<String> {
  let mut args = vec!["exec", container_id];
  args.extend_from_slice(cmd);
  run(app, "docker", &args).await
}

/// Streams `docker logs -f <container_id>` line-by-line.
#[allow(dead_code)]
pub fn logs_stream<R: Runtime>(app: &AppHandle<R>, container_id: &str) -> Result<SpawnedProcess> {
  let args = vec!["logs".to_string(), "-f".to_string(), container_id.to_string()];
  Ok(crate::process::spawn(app, "docker", &args, None)?)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainerStats {
  pub cpu_percent: f64,
  pub memory_mb: f64,
  pub network_rx_bytes: f64,
  pub network_tx_bytes: f64,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct RawStats {
  #[serde(rename = "CPUPerc")]
  cpu_perc: String,
  #[serde(rename = "MemUsage")]
  mem_usage: String,
  #[serde(rename = "NetIO")]
  net_io: String,
}

/// `docker stats --no-stream`, parsed into numeric values.
#[allow(dead_code)]
pub async fn stats<R: Runtime>(app: &AppHandle<R>, container_id: &str) -> Result<ContainerStats> {
  let raw = run(app, "docker", &["stats", "--no-stream", "--format", "{{json .}}", container_id]).await?;
  let parsed: RawStats = serde_json::from_str(&raw).map_err(|e| Error::ParseFailed(e.to_string()))?;
  let (network_rx_bytes, network_tx_bytes) = parse_net_io(&parsed.net_io)?;
  Ok(ContainerStats {
    cpu_percent: parse_percent(&parsed.cpu_perc)?,
    memory_mb: parse_mem_usage_mb(&parsed.mem_usage)?,
    network_rx_bytes,
    network_tx_bytes,
  })
}

#[allow(dead_code)]
fn parse_percent(raw: &str) -> Result<f64> {
  raw
    .trim()
    .trim_end_matches('%')
    .parse::<f64>()
    .map_err(|_| Error::ParseFailed(format!("bad percent value: {raw}")))
}

/// Parses a docker byte-size string like "12.5MiB", "850B", "1.2GB" into
/// bytes. Docker uses binary units (KiB/MiB/GiB) for memory and decimal
/// units (kB/MB/GB) for network I/O — both are accepted.
fn parse_byte_size(raw: &str) -> Result<f64> {
  let raw = raw.trim();
  let unit_start = raw.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(raw.len());
  let (number, unit) = raw.split_at(unit_start);
  let value: f64 = number.parse().map_err(|_| Error::ParseFailed(format!("bad size value: {raw}")))?;
  let multiplier = match unit.trim() {
    "B" | "" => 1.0,
    "kB" => 1_000.0,
    "KiB" => 1_024.0,
    "MB" => 1_000_000.0,
    "MiB" => 1_024.0 * 1_024.0,
    "GB" => 1_000_000_000.0,
    "GiB" => 1_024.0 * 1_024.0 * 1_024.0,
    other => return Err(Error::ParseFailed(format!("unknown size unit: {other}"))),
  };
  Ok(value * multiplier)
}

#[allow(dead_code)]
fn parse_mem_usage_mb(raw: &str) -> Result<f64> {
  let used = raw.split('/').next().ok_or_else(|| Error::ParseFailed(format!("bad mem usage: {raw}")))?;
  Ok(parse_byte_size(used)? / (1024.0 * 1024.0))
}

#[allow(dead_code)]
fn parse_net_io(raw: &str) -> Result<(f64, f64)> {
  let mut parts = raw.split('/');
  let rx = parts.next().ok_or_else(|| Error::ParseFailed(format!("bad net io: {raw}")))?;
  let tx = parts.next().ok_or_else(|| Error::ParseFailed(format!("bad net io: {raw}")))?;
  Ok((parse_byte_size(rx)?, parse_byte_size(tx)?))
}

/// Clones a (possibly local) git repo into `target_dir`. Used for clone-mode
/// sandboxes — `source_path` is the project's existing local `repo_path`,
/// so this works entirely offline via git's local-filesystem clone support.
pub async fn clone_repo<R: Runtime>(app: &AppHandle<R>, source_path: &str, target_dir: &Path) -> Result<()> {
  run(app, "git", &["clone", source_path, &target_dir.to_string_lossy()]).await?;
  Ok(())
}

/// Asks the OS for an unused local port by binding to port 0. There's a
/// small race between this returning and the caller actually using the
/// port, but it's the standard best-effort approach for this purpose.
pub fn find_free_port() -> Result<u16> {
  let listener = TcpListener::bind("127.0.0.1:0")?;
  Ok(listener.local_addr()?.port())
}

/// Free host RAM in megabytes, for the pre-start memory check.
pub fn host_free_memory_mb() -> f64 {
  let mut sys = sysinfo::System::new();
  sys.refresh_memory();
  sys.available_memory() as f64 / (1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_cpu_percent() {
    assert_eq!(parse_percent("12.34%").unwrap(), 12.34);
    assert_eq!(parse_percent("0.00%").unwrap(), 0.0);
  }

  #[test]
  fn parses_mem_usage() {
    assert_eq!(parse_mem_usage_mb("12.5MiB / 512MiB").unwrap(), 12.5);
    assert!((parse_mem_usage_mb("1GiB / 2GiB").unwrap() - 1024.0).abs() < 0.001);
  }

  #[test]
  fn parses_net_io() {
    let (rx, tx) = parse_net_io("1.2kB / 850B").unwrap();
    assert_eq!(rx, 1200.0);
    assert_eq!(tx, 850.0);
  }

  #[test]
  fn rejects_unknown_unit() {
    assert!(parse_byte_size("5XB").is_err());
  }

  #[test]
  fn finds_a_free_port() {
    let port = find_free_port().unwrap();
    assert!(port > 0);
  }

  #[test]
  fn reports_free_memory() {
    assert!(host_free_memory_mb() > 0.0);
  }
}
