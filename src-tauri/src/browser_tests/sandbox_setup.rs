//! The sandbox side of browser testing: the `overnight-browser-test`
//! helper, the Claude skill that teaches the agent to use it, and the
//! network rule that lets the sandbox reach the server.

use tauri::{AppHandle, Runtime};

use super::SERVER_PORT;
use crate::sbx::SandboxFile;

const HELPER_PATH: &str = "/home/agent/.local/bin/overnight-browser-test";
const HELPER: &str = include_str!("overnight-browser-test.sh");
const SKILL_DIR: &str = "/home/agent/.claude/skills/browser-test";
const SKILL_PATH: &str = "/home/agent/.claude/skills/browser-test/SKILL.md";
const SKILL: &str = include_str!("SKILL.md");

/// The sbx proxy rewrites `host.docker.internal` to `localhost`, so the
/// allow rule names localhost.
pub fn network_rule() -> String {
  format!("localhost:{SERVER_PORT}")
}

/// Installs (or refreshes) the helper, plus the skill for Claude sandboxes.
pub async fn install<R: Runtime>(app: &AppHandle<R>, name: &str, agent: &str) -> crate::sbx::Result<()> {
  let mut files = vec![SandboxFile { path: HELPER_PATH, content: HELPER, executable: true }];
  if agent == crate::agents::CLAUDE.id {
    files.push(SandboxFile { path: SKILL_PATH, content: SKILL, executable: false });
  }
  crate::sbx::write_files(app, name, "Install browser test helper", &files).await
}

pub async fn uninstall<R: Runtime>(app: &AppHandle<R>, name: &str) -> crate::sbx::Result<()> {
  crate::sbx::remove_paths(app, name, "Remove browser test helper", &[HELPER_PATH, SKILL_DIR]).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn skill_has_frontmatter_claude_can_load() {
    assert!(SKILL.starts_with("---\nname: browser-test\ndescription: "));
  }

  #[test]
  fn helper_reads_the_env_vars_overnight_exports() {
    for var in ["OVERNIGHT_BROWSER_URL", "OVERNIGHT_BROWSER_TOKEN", "OVERNIGHT_BROWSER_PORT"] {
      assert!(HELPER.contains(var), "{var}");
    }
    assert_eq!(network_rule(), "localhost:47800");
  }
}
