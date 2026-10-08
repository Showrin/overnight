//! Claude in Chrome browser testing for sandboxes.
//!
//! A sandbox can't run Chrome, and can't reach the host's Chrome either. So
//! when a sandbox has Claude in Chrome enabled, its agent writes a test doc
//! and POSTs it to `server` (reachable from inside the sandbox as
//! `host.docker.internal:SERVER_PORT`). The test is stored in
//! `db::browser_tests`, a host-side `claude --chrome` agent runs it against
//! the sandbox's published app port (or an external host URL), and the
//! sandbox polls the same server for the report.

pub mod chrome;
pub mod runner;
pub mod sandbox_setup;
pub mod server;

use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use crate::db::models::BrowserTest;
use crate::db::DbPool;

/// Host port the browser-test server listens on (loopback only).
pub const SERVER_PORT: u16 = 47800;

/// In-sandbox app port used when neither the request nor the sandbox's
/// settings name one. Matches the port every sandbox publishes at create.
pub const DEFAULT_SANDBOX_APP_PORT: u16 = 8080;

/// Tauri event emitted whenever a browser test changes, with the test as
/// its payload, so the Browser tab can refresh.
pub const CHANGED_EVENT: &str = "browser-tests-changed";

/// Shared between the HTTP server, the runner, and Tauri commands.
#[derive(Clone)]
pub struct Hub {
  pub pool: DbPool,
  wake: Arc<Notify>,
  on_change: Arc<dyn Fn(&BrowserTest) + Send + Sync>,
  /// The test the runner is executing right now, and how to stop it.
  running: Arc<Mutex<Option<(String, tokio::sync::oneshot::Sender<()>)>>>,
}

impl Hub {
  pub fn new(pool: DbPool, on_change: impl Fn(&BrowserTest) + Send + Sync + 'static) -> Self {
    Self { pool, wake: Arc::new(Notify::new()), on_change: Arc::new(on_change), running: Arc::default() }
  }

  /// Reports a change to the UI and wakes the runner in case a test was
  /// just queued.
  pub fn changed(&self, test: &BrowserTest) {
    (self.on_change)(test);
    self.wake.notify_one();
  }

  pub async fn wait_for_work(&self) {
    self.wake.notified().await;
  }

  fn set_running(&self, id: &str, cancel: tokio::sync::oneshot::Sender<()>) {
    *self.running.lock().unwrap() = Some((id.to_string(), cancel));
  }

  fn clear_running(&self, id: &str) {
    let mut running = self.running.lock().unwrap();
    if running.as_ref().is_some_and(|(current, _)| current == id) {
      *running = None;
    }
  }

  /// Cancels a test: drops it from the queue if it hasn't started, or
  /// kills its host agent if it's running.
  pub fn cancel(&self, id: &str) -> Result<BrowserTest, String> {
    let conn = self.pool.get().map_err(|e| e.to_string())?;
    let test = crate::db::browser_tests::get(&conn, id).map_err(|e| e.to_string())?;
    let test = if test.status == "running" {
      crate::db::browser_tests::mark_cancelled(&conn, id).map_err(|e| e.to_string())?;
      let mut running = self.running.lock().unwrap();
      if running.as_ref().is_some_and(|(current, _)| current == id) {
        if let Some((_, cancel)) = running.take() {
          let _ = cancel.send(());
        }
      }
      crate::db::browser_tests::get(&conn, id).map_err(|e| e.to_string())?
    } else {
      crate::db::browser_tests::cancel_pending(&conn, id).map_err(|e| e.to_string())?
    };
    self.changed(&test);
    Ok(test)
  }
}
