//! Restoring a backup's `.claude` and/or `.git` copy back into a running
//! sandbox. Mirrors `backup.rs`'s active-operation tracking pattern so the
//! frontend can show restore progress the same way it shows backup progress.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::backup::BackupScope;
use crate::db::models::now_millis;
use crate::db::{backups, sandboxes, DbPool};

#[derive(Debug, Clone, Serialize)]
pub struct ActiveRestore {
  pub target_sandbox_id: String,
  pub source_sandbox_id: String,
  pub scope: String,
  pub started_at: i64,
}

/// Restores in flight, keyed by a fresh id per restore (not sandbox id —
/// unlike backups, nothing about a restore is naturally one-per-sandbox).
#[derive(Default)]
pub struct RestoreState {
  active: HashMap<String, ActiveRestore>,
}

pub fn active_restores(app: &AppHandle) -> Vec<ActiveRestore> {
  let Some(state) = app.try_state::<Mutex<RestoreState>>() else {
    return Vec::new();
  };
  let Ok(state) = state.lock() else {
    return Vec::new();
  };
  state.active.values().cloned().collect()
}

struct ActiveRestoreGuard {
  app: AppHandle,
  id: String,
}

impl Drop for ActiveRestoreGuard {
  fn drop(&mut self) {
    if let Some(state) = self.app.try_state::<Mutex<RestoreState>>() {
      if let Ok(mut state) = state.lock() {
        state.active.remove(&self.id);
      }
    }
  }
}

fn scope_label(scope: BackupScope) -> &'static str {
  match scope {
    BackupScope::Claude => "claude",
    BackupScope::Codex => "codex",
    BackupScope::Git => "git",
    BackupScope::All => "all",
  }
}

/// Pushes a backup's `.claude` and/or `.git` copy back into a running
/// sandbox, per `scope`. The two halves are restored independently — only
/// the requested, present half(ves) are copied.
pub async fn restore_backup(
  app: &AppHandle,
  pool: &DbPool,
  backup_id: &str,
  target_sandbox_id: &str,
  scope: BackupScope,
) -> std::result::Result<(), String> {
  let (backup, target) = {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let backup = backups::get(&conn, backup_id).map_err(|e| e.to_string())?;
    let target = sandboxes::get(&conn, target_sandbox_id).map_err(|e| e.to_string())?;
    (backup, target)
  };
  if target.status != "running" {
    return Err("target sandbox must be running to restore a backup".to_string());
  }
  let name = target.sbx_name.ok_or_else(|| "target sandbox has no sbx sandbox yet".to_string())?;

  let op_id = crate::db::models::new_id();
  if let Some(state) = app.try_state::<Mutex<RestoreState>>() {
    if let Ok(mut state) = state.lock() {
      state.active.insert(
        op_id.clone(),
        ActiveRestore {
          target_sandbox_id: target_sandbox_id.to_string(),
          source_sandbox_id: backup.sandbox_id.clone(),
          scope: scope_label(scope).to_string(),
          started_at: now_millis(),
        },
      );
    }
  }
  let _guard = ActiveRestoreGuard { app: app.clone(), id: op_id };

  if scope.wants_agent_home() {
    let agent_kit = crate::agents::get(&target.agent).ok_or_else(|| format!("unknown agent: {}", target.agent))?;
    if let Some(requested) = scope.named_agent() {
      if requested != target.agent {
        return Err(format!("target sandbox's agent is \"{}\", not \"{requested}\"", target.agent));
      }
    }
    let has_agent_data = match agent_kit.id {
      "codex" => backup.has_codex,
      _ => backup.has_claude,
    };
    if !has_agent_data {
      return Err(format!("this backup has no .{} copy", agent_kit.id));
    }
    let host_src = format!("{}/{}", backup.host_dir, agent_kit.id);
    let mounted_dirs = match agent_kit.id {
      "codex" => crate::sbx::CODEX_HOME_MOUNTED_DIRS,
      _ => crate::sbx::CLAUDE_HOME_MOUNTED_DIRS,
    };
    crate::sbx::restore_directory(app, &name, &host_src, agent_kit.home_dir, mounted_dirs)
      .await
      .map_err(|e| e.to_string())?;
  }

  if scope.wants_git() {
    if !backup.has_git {
      return Err("this backup has no .git copy".to_string());
    }
    let workspace = crate::sbx::workspace_path(app, &name).await.map_err(|e| e.to_string())?;
    let workspace = workspace.ok_or_else(|| "target sandbox has no workspace path".to_string())?;
    let git_path = format!("{workspace}/.git");
    let host_src = format!("{}/git", backup.host_dir);
    crate::sbx::restore_directory(app, &name, &host_src, &git_path, &[]).await.map_err(|e| e.to_string())?;
  }

  Ok(())
}
