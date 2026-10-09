//! The host side of an external-target browser test: optionally starts the
//! user's server commands from the project folder, waits for the app (and
//! each server's ready URL) to answer, and stops every process tree it
//! started afterwards.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::db::models::HostServer;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const LOG_TAIL_LINES: usize = 40;

/// Servers this run started (none when the user runs their own), plus the
/// URLs that must answer before testing.
#[derive(Default)]
pub struct HostServers {
  servers: Vec<(String, Child, PathBuf)>,
  ready_urls: Vec<String>,
}

impl HostServers {
  /// Starts every server in `cwd`, logging each one's output to a file in
  /// `log_dir`. Stops whatever already started if a later one can't spawn.
  pub fn start(servers: &[HostServer], cwd: &Path, log_dir: &Path) -> Result<Self, String> {
    let mut started = Self::default();
    for (i, server) in servers.iter().enumerate() {
      let command = &server.command;
      started.ready_urls.extend(server.ready_url.clone());
      let log_path = log_dir.join(format!("server-{}.log", i + 1));
      let log = File::create(&log_path).map_err(|e| format!("couldn't create {}: {e}", log_path.display()))?;
      let log_err = log.try_clone().map_err(|e| e.to_string())?;
      let mut cmd = shell_command(command);
      cmd.current_dir(cwd).stdin(Stdio::null()).stdout(log).stderr(log_err);
      let child = cmd.spawn().map_err(|e| format!("couldn't run `{command}`: {e}"))?;
      started.servers.push((command.clone(), child, log_path));
    }
    Ok(started)
  }

  /// Polls `url` and every server's ready URL until each gives any HTTP
  /// response. Fails early, with the log tail, if a started server exits
  /// first, and on `timeout`.
  pub async fn wait_until_up(&mut self, url: &str, timeout: Duration) -> Result<(), String> {
    let mut pending: Vec<String> = std::iter::once(url.to_string()).chain(self.ready_urls.iter().cloned()).collect();
    pending.dedup();
    let deadline = Instant::now() + timeout;
    loop {
      for (command, child, log) in &mut self.servers {
        if let Ok(Some(status)) = child.try_wait() {
          return Err(format!("`{command}` exited ({status}) before {} came up:\n{}", pending.join(", "), log_tail(log)));
        }
      }
      let mut still_down = Vec::new();
      for url in pending {
        if !is_up(&url).await {
          still_down.push(url);
        }
      }
      pending = still_down;
      if pending.is_empty() {
        return Ok(());
      }
      if Instant::now() >= deadline {
        let logs: Vec<String> = self.servers.iter().map(|(command, _, log)| format!("`{command}`:\n{}", log_tail(log))).collect();
        return Err(format!("{} didn't respond within {}s.\n{}", pending.join(", "), timeout.as_secs(), logs.join("\n")).trim_end().to_string());
      }
      tokio::time::sleep(POLL_INTERVAL).await;
    }
  }
}

/// Whether `url` gives any HTTP response at all — a 404 or 500 still means
/// the server is up.
pub async fn is_up(url: &str) -> bool {
  let Ok(client) = reqwest::Client::builder().timeout(Duration::from_secs(3)).danger_accept_invalid_certs(true).build() else {
    return false;
  };
  client.get(url).send().await.is_ok()
}

/// Stops each server's whole process tree — `pnpm dev` and friends spawn
/// children that outlive their shell otherwise.
impl Drop for HostServers {
  fn drop(&mut self) {
    for (command, child, _) in &mut self.servers {
      if matches!(child.try_wait(), Ok(Some(_))) {
        continue;
      }
      kill_tree(child.id());
      let _ = child.kill();
      let _ = child.wait();
      log::info!("browser tests: stopped `{command}`");
    }
  }
}

#[cfg(target_os = "windows")]
fn shell_command(command: &str) -> Command {
  use std::os::windows::process::CommandExt;
  let mut cmd = Command::new("cmd");
  // `/S /C "<cmd>"`: cmd strips exactly the outer quotes and runs the rest
  // verbatim, so the user's own quoting survives.
  cmd.raw_arg(format!("/S /C \"{command}\""));
  crate::git::hide_console(&mut cmd);
  cmd
}

#[cfg(not(target_os = "windows"))]
fn shell_command(command: &str) -> Command {
  use std::os::unix::process::CommandExt;
  let mut cmd = Command::new("sh");
  cmd.args(["-c", command]).process_group(0);
  cmd
}

#[cfg(target_os = "windows")]
fn kill_tree(pid: u32) {
  let mut cmd = Command::new("taskkill");
  cmd.args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null());
  crate::git::hide_console(&mut cmd);
  let _ = cmd.status();
}

#[cfg(not(target_os = "windows"))]
fn kill_tree(pid: u32) {
  // The server was started as its own process group leader, so -pid
  // addresses the whole group.
  let _ = Command::new("kill").args(["-TERM", &format!("-{pid}")]).status();
}

fn log_tail(path: &Path) -> String {
  let text = std::fs::read_to_string(path).unwrap_or_default();
  let lines: Vec<&str> = text.lines().collect();
  lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("overnight-host-servers-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
  }

  #[cfg(target_os = "windows")]
  const LONG_RUNNING: &str = "ping -n 60 127.0.0.1 > NUL";
  #[cfg(not(target_os = "windows"))]
  const LONG_RUNNING: &str = "sleep 60";

  fn server(command: &str, ready_url: Option<&str>) -> HostServer {
    HostServer { command: command.to_string(), ready_url: ready_url.map(String::from) }
  }

  async fn serve_ok() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
    tokio::spawn(async move { axum::serve(listener, app).await });
    url
  }

  #[tokio::test]
  async fn reports_a_server_that_exits_early_with_its_output() {
    let dir = temp_dir();
    let mut servers = HostServers::start(&[server("echo \"boom from server\" && exit 3", None)], &dir, &dir).unwrap();
    let err = servers.wait_until_up("http://127.0.0.1:1", Duration::from_secs(20)).await.unwrap_err();
    assert!(err.contains("exited"), "{err}");
    assert!(err.contains("boom from server"), "{err}");
  }

  #[tokio::test]
  async fn waits_for_the_url_then_stops_the_servers() {
    let dir = temp_dir();
    let url = serve_ok().await;

    let mut servers = HostServers::start(&[server(LONG_RUNNING, None)], &dir, &dir).unwrap();
    servers.wait_until_up(&url, Duration::from_secs(20)).await.unwrap();
    let pid = servers.servers[0].1.id();
    drop(servers);

    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    assert!(sys.process(sysinfo::Pid::from_u32(pid)).is_none(), "server shell still running");
  }

  #[tokio::test]
  async fn times_out_with_logs() {
    let dir = temp_dir();
    let mut servers = HostServers::start(&[server(LONG_RUNNING, None)], &dir, &dir).unwrap();
    let err = servers.wait_until_up("http://127.0.0.1:1", Duration::from_secs(2)).await.unwrap_err();
    assert!(err.contains("didn't respond within 2s"), "{err}");
  }

  #[tokio::test]
  async fn also_waits_for_each_servers_ready_url() {
    let dir = temp_dir();
    let app_url = serve_ok().await;
    let mut servers = HostServers::start(&[server(LONG_RUNNING, Some("http://127.0.0.1:1/health"))], &dir, &dir).unwrap();
    let err = servers.wait_until_up(&app_url, Duration::from_secs(2)).await.unwrap_err();
    assert!(err.starts_with("http://127.0.0.1:1/health didn't respond"), "{err}");

    let api_url = serve_ok().await;
    let mut servers = HostServers::start(&[server(LONG_RUNNING, Some(&api_url))], &dir, &dir).unwrap();
    servers.wait_until_up(&app_url, Duration::from_secs(10)).await.unwrap();
  }

  #[tokio::test]
  async fn without_servers_just_waits_for_the_url() {
    let url = serve_ok().await;
    HostServers::default().wait_until_up(&url, Duration::from_secs(10)).await.unwrap();
    assert!(!is_up("http://127.0.0.1:1").await);
  }
}
