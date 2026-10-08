//! Claude in Chrome browser testing for sandboxes.
//!
//! A sandbox can't run Chrome, and can't reach the host's Chrome either. So
//! when a sandbox has Claude in Chrome enabled, its agent writes a test doc
//! and POSTs it to `server` (reachable from inside the sandbox as
//! `host.docker.internal:SERVER_PORT`). The test is stored in
//! `db::browser_tests`, a host-side `claude --chrome` agent runs it against
//! the sandbox's published app port (or an external host URL), and the
//! sandbox polls the same server for the report.

pub mod server;

use std::sync::Arc;

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
}

impl Hub {
  pub fn new(pool: DbPool, on_change: impl Fn(&BrowserTest) + Send + Sync + 'static) -> Self {
    Self { pool, wake: Arc::new(Notify::new()), on_change: Arc::new(on_change) }
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
}
