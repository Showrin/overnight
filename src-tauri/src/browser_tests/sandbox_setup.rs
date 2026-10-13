//! The sandbox side of browser testing: the `overnight-browser-test`
//! helper and the guide that teaches the agent to use it, both kept in the
//! sandbox's own `~/.overnight`, a pointer to the guide in the agent's
//! global instructions file, and the network rule that lets the sandbox
//! reach the server.
//!
//! Nothing goes in `~/.claude/skills`: sbx mounts its shared skills store
//! there, so a skill written there would reach every sandbox — and some
//! machines mount it read-only.

use tauri::{AppHandle, Runtime};

use super::SERVER_PORT;
use crate::agents::AgentKit;
use crate::sbx::{managed_block_script, shell_quote, write_files_script, SandboxFile};

const SANDBOX_HOME: &str = "/home/agent";
const OVERNIGHT_DIR: &str = ".overnight";
const HELPER: &str = include_str!("overnight-browser-test.sh");
const GUIDE: &str = include_str!("browser-testing-guide.md");
/// Where an earlier version installed the guide as a skill, which landed
/// in sbx's shared skills store. Removed on every install/uninstall where
/// the store is writable.
const LEGACY_SKILL_DIR: &str = ".claude/skills/browser-test";
const BLOCK_START: &str = "<!-- overnight:browser-testing:start -->";
const BLOCK_END: &str = "<!-- overnight:browser-testing:end -->";
const INSTRUCTIONS: &str = "## Browser testing

You can test this project's web frontend in a real Chrome browser on the user's machine, through Overnight. After building or changing anything user-facing in a web app, read `~/.overnight/browser-testing.md` and follow it before calling the work done.";

/// The sbx proxy rewrites `host.docker.internal` to `localhost`, so the
/// allow rule names localhost.
pub fn network_rule() -> String {
  format!("localhost:{SERVER_PORT}")
}

/// Installs (or refreshes) the helper and guide, and points the agent at
/// them from its instructions file.
pub async fn install<R: Runtime>(app: &AppHandle<R>, name: &str, agent: &str) -> crate::sbx::Result<()> {
  let script = install_script(SANDBOX_HOME, kit(agent));
  crate::sbx::exec_script(app, name, "Install browser test helper", &script).await
}

pub async fn uninstall<R: Runtime>(app: &AppHandle<R>, name: &str, agent: &str) -> crate::sbx::Result<()> {
  let script = uninstall_script(SANDBOX_HOME, kit(agent));
  crate::sbx::exec_script(app, name, "Remove browser test helper", &script).await
}

fn kit(agent: &str) -> Option<&'static AgentKit> {
  crate::agents::get(agent)
}

/// The agent's global instructions file, e.g. `~/.claude/CLAUDE.md`.
fn instructions_path(home: &str, kit: &AgentKit) -> String {
  let agent_dir = kit.home_dir.strip_prefix(SANDBOX_HOME).unwrap_or(kit.home_dir).trim_start_matches('/');
  format!("{home}/{agent_dir}/{}", kit.instructions_file)
}

fn legacy_cleanup(home: &str) -> String {
  format!("rm -rf {} 2>/dev/null || true\n", shell_quote(&format!("{home}/{LEGACY_SKILL_DIR}")))
}

/// `home` is the sandbox's home dir; tests point it at a temp folder.
fn install_script(home: &str, kit: Option<&AgentKit>) -> String {
  // A CRLF checkout would break the script under bash; .gitattributes
  // prevents that, this is the backstop.
  let helper = HELPER.replace("\r\n", "\n");
  let guide = GUIDE.replace("\r\n", "\n");
  let helper_path = format!("{home}/{OVERNIGHT_DIR}/bin/overnight-browser-test");
  let guide_path = format!("{home}/{OVERNIGHT_DIR}/browser-testing.md");
  let mut script = legacy_cleanup(home);
  script.push_str(&write_files_script(&[
    SandboxFile { path: &helper_path, content: &helper, executable: true },
    SandboxFile { path: &guide_path, content: &guide, executable: false },
  ]));
  if let Some(kit) = kit {
    script.push_str(&managed_block_script(&instructions_path(home, kit), BLOCK_START, BLOCK_END, Some(INSTRUCTIONS)));
  }
  script
}

fn uninstall_script(home: &str, kit: Option<&AgentKit>) -> String {
  let mut script = legacy_cleanup(home);
  script.push_str(&format!("rm -rf {}\n", shell_quote(&format!("{home}/{OVERNIGHT_DIR}"))));
  if let Some(kit) = kit {
    script.push_str(&managed_block_script(&instructions_path(home, kit), BLOCK_START, BLOCK_END, None));
  }
  script
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;
  use std::process::Command;

  #[test]
  fn helper_reads_the_env_vars_overnight_exports() {
    for var in ["OVERNIGHT_BROWSER_URL", "OVERNIGHT_BROWSER_TOKEN", "OVERNIGHT_BROWSER_PORT"] {
      assert!(HELPER.contains(var), "{var}");
    }
    assert_eq!(network_rule(), "localhost:47800");
  }

  #[test]
  fn instructions_go_in_each_agents_own_file() {
    assert_eq!(instructions_path("/home/agent", &crate::agents::CLAUDE), "/home/agent/.claude/CLAUDE.md");
    assert_eq!(instructions_path("/home/agent", &crate::agents::CODEX), "/home/agent/.codex/AGENTS.md");
    assert_eq!(instructions_path("home", &crate::agents::CLAUDE), "home/.claude/CLAUDE.md");
  }

  #[test]
  fn never_writes_into_the_shared_skills_store() {
    let script = install_script("/home/agent", Some(&crate::agents::CLAUDE));
    assert!(!script.contains("/home/agent/.claude/skills/browser-test/"), "{script}");
    assert!(script.contains("rm -rf '/home/agent/.claude/skills/browser-test' 2>/dev/null || true"));
  }

  fn bash_available() -> bool {
    Command::new("bash").arg("--version").output().is_ok_and(|o| o.status.success())
  }

  /// Runs from a file in `dir` with relative paths: on Windows, `bash` may
  /// be WSL's, which mangles `-c` arguments and can't see `C:/` paths.
  fn run_in(dir: &Path, script: &str) {
    std::fs::write(dir.join("script.sh"), format!("set -e\n{script}")).unwrap();
    assert!(Command::new("bash").current_dir(dir).arg("script.sh").status().unwrap().success(), "{script}");
  }

  #[test]
  fn install_then_uninstall_leaves_the_users_own_instructions() {
    if !bash_available() {
      return;
    }
    let dir = std::env::temp_dir().join(format!("overnight-setup-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("home/.claude/skills/browser-test")).unwrap();
    std::fs::write(dir.join("home/.claude/CLAUDE.md"), "# My rules\n").unwrap();

    run_in(&dir, &install_script("home", Some(&crate::agents::CLAUDE)));
    run_in(&dir, &install_script("home", Some(&crate::agents::CLAUDE)));
    let helper = std::fs::read_to_string(dir.join("home/.overnight/bin/overnight-browser-test")).unwrap();
    assert!(helper.starts_with("#!/usr/bin/env bash\n"));
    assert!(!helper.contains('\r'));
    assert!(std::fs::read_to_string(dir.join("home/.overnight/browser-testing.md")).unwrap().contains("overnight-browser-test check"));
    let claude_md = std::fs::read_to_string(dir.join("home/.claude/CLAUDE.md")).unwrap();
    assert!(claude_md.starts_with("# My rules\n\n<!-- overnight:browser-testing:start -->\n## Browser testing"), "{claude_md}");
    assert_eq!(claude_md.matches(BLOCK_START).count(), 1, "{claude_md}");
    assert!(!dir.join("home/.claude/skills/browser-test").exists());

    run_in(&dir, &uninstall_script("home", Some(&crate::agents::CLAUDE)));
    assert_eq!(std::fs::read_to_string(dir.join("home/.claude/CLAUDE.md")).unwrap(), "# My rules\n");
    assert!(!dir.join("home/.overnight").exists());
    let _ = std::fs::remove_dir_all(&dir);
  }
}
