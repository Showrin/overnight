//! Runs Git Sync on running clone-mode sandboxes that have an auto-sync interval.

use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::backup::is_due;
use crate::db::models::{now_millis, Sandbox};
use crate::db::{sandboxes, DbPool};

const TICK_SECS: u64 = 60;

#[derive(Serialize, Clone)]
struct AutoSyncEvent {
  sandbox_id: String,
  outcomes: Vec<crate::git::BranchSyncOutcome>,
}

fn due_sandboxes(list: &[Sandbox], now: i64, attempts: &HashMap<String, i64>) -> Vec<String> {
  list
    .iter()
    .filter(|s| s.status == "running" && s.mode == "clone" && s.sbx_name.is_some())
    .filter_map(|s| {
      let interval = s.git_sync_interval_minutes?;
      let last = s.last_git_sync_at.unwrap_or(s.created_at);
      is_due(now, last, attempts.get(&s.id).copied(), interval).then(|| s.id.clone())
    })
    .collect()
}

pub async fn run(app: AppHandle, pool: DbPool) {
  let mut attempts: HashMap<String, i64> = HashMap::new();
  loop {
    tokio::time::sleep(std::time::Duration::from_secs(TICK_SECS)).await;
    let list = match pool.get().map_err(|e| e.to_string()).and_then(|conn| sandboxes::list(&conn).map_err(|e| e.to_string())) {
      Ok(list) => list,
      Err(e) => {
        log::warn!("git sync scheduler: {e}");
        continue;
      }
    };
    let now = now_millis();
    for id in due_sandboxes(&list, now, &attempts) {
      attempts.insert(id.clone(), now);
      let task_pool = pool.clone();
      let task_id = id.clone();
      match tokio::task::spawn_blocking(move || crate::commands::git_sync_core(&task_pool, &task_id)).await {
        Ok(Ok(outcomes)) => {
          if let Err(e) = app.emit("git-sync-auto", AutoSyncEvent { sandbox_id: id, outcomes }) {
            log::warn!("git sync scheduler: emit failed: {e}");
          }
        }
        Ok(Err(e)) => log::warn!("git sync scheduler: sync failed for {id}: {e}"),
        Err(e) => log::warn!("git sync scheduler: task failed for {id}: {e}"),
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::projects;

  #[test]
  fn picks_only_due_running_clone_sandboxes_with_interval() {
    let conn = test_conn();
    let p1 = projects::create(&conn, "P1", "/r1").unwrap();
    let p2 = projects::create(&conn, "P2", "/r2").unwrap();
    let p3 = projects::create(&conn, "P3", "/r3").unwrap();
    let due = sandboxes::create(&conn, &p1.id, "clone", None, None, "default", None, "claude").unwrap();
    sandboxes::update_status(&conn, &due.id, "running", Some("due")).unwrap();
    let due = sandboxes::set_git_sync_interval(&conn, &due.id, Some(15)).unwrap();
    let off = sandboxes::create(&conn, &p2.id, "clone", None, None, "default", None, "claude").unwrap();
    let off = sandboxes::update_status(&conn, &off.id, "running", Some("off")).unwrap();
    let mount = sandboxes::create(&conn, &p3.id, "mount", Some("/r3"), None, "default", None, "claude").unwrap();
    sandboxes::update_status(&conn, &mount.id, "running", Some("mount")).unwrap();
    let mount = sandboxes::set_git_sync_interval(&conn, &mount.id, Some(15)).unwrap();

    let list = vec![due.clone(), off, mount];
    let later = due.created_at + 16 * 60_000;
    assert_eq!(due_sandboxes(&list, later, &HashMap::new()), vec![due.id.clone()]);
    assert!(due_sandboxes(&list, due.created_at + 60_000, &HashMap::new()).is_empty());
    let attempts = HashMap::from([(due.id.clone(), later)]);
    assert!(due_sandboxes(&list, later + 60_000, &attempts).is_empty());
  }
}
