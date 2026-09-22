//! Repository of per-agent configuration ("coding CLI to run inside a
//! sandbox") — the single place that knows Claude Code's and Codex's CLI
//! token, home directory, secret service, and permission-mode shape.
//! Every other module (sandbox creation, backups, plan sync, the Settings
//! UI) looks values up here instead of assuming "claude" — add a new
//! `AgentKit` entry here (and to the frontend's `AGENTS` list in
//! `src/lib/agentHost.ts`) to support another agent.

pub struct AgentKit {
  /// Stored in `sandboxes.agent`; also the wire value the frontend sends.
  pub id: &'static str,
  pub label: &'static str,
  /// `sbx create --name <name> [--clone] <cli_token> <workspace>` and
  /// `sbx run --name <name> <cli_token>` both take this token.
  pub cli_token: &'static str,
  /// This agent's home directory inside the sandbox — the backup/plan
  /// source and the destination for `host_auth_relative_path`.
  pub home_dir: &'static str,
  /// `sbx secret set <secret_service> ...` — must be one of
  /// `commands::KNOWN_SECRET_SERVICES`.
  #[allow(dead_code)] // not consumed yet — Codex/Claude auth goes through the generic secrets UI today, not this field
  pub secret_service: &'static str,
  /// The CLI flag `set_default_permission_mode` appends after `cli_token`
  /// in the `/etc/sandbox-persistent.sh` alias.
  pub permission_flag: &'static str,
  pub permission_modes: &'static [&'static str],
  pub default_permission_mode: &'static str,
  /// `settings` table key this agent's default permission mode is stored
  /// under, e.g. `"default_claude_permission_mode"`.
  pub permission_mode_settings_key: &'static str,
  /// Path under the host's home directory to copy into `home_dir` (same
  /// basename) when a sandbox is created, or `None` if this agent
  /// authenticates purely via `secret_service` (Claude Code does — it has
  /// no host auth file this app copies in, unlike Codex's local
  /// `codex login` session file).
  pub host_auth_relative_path: Option<&'static str>,
}

pub const CLAUDE: AgentKit = AgentKit {
  id: "claude",
  label: "Claude",
  cli_token: "claude",
  home_dir: "/home/agent/.claude",
  secret_service: "anthropic",
  permission_flag: "--permission-mode",
  permission_modes: &["plan", "default", "acceptEdits", "bypassPermissions"],
  default_permission_mode: "default",
  permission_mode_settings_key: "default_claude_permission_mode",
  host_auth_relative_path: None,
};

/// **UNVERIFIED** — no real Codex CLI install is available in this dev
/// environment. `cli_token`/`permission_flag`/`permission_modes` are a
/// best-effort reading of Codex's public `--ask-for-approval` flag
/// (`untrusted` | `on-failure` | `on-request` | `never`); confirm against
/// a real `codex --help` before shipping. `host_auth_relative_path` mirrors
/// where `codex login` is documented to persist its session
/// (`~/.codex/auth.json`) — confirm against a real `codex login` run.
pub const CODEX: AgentKit = AgentKit {
  id: "codex",
  label: "Codex",
  cli_token: "codex",
  home_dir: "/home/agent/.codex",
  secret_service: "openai",
  permission_flag: "--ask-for-approval",
  permission_modes: &["untrusted", "on-failure", "on-request", "never"],
  default_permission_mode: "on-request",
  permission_mode_settings_key: "default_codex_permission_mode",
  host_auth_relative_path: Some(".codex/auth.json"),
};

pub const ALL: &[&AgentKit] = &[&CLAUDE, &CODEX];

pub fn get(id: &str) -> Option<&'static AgentKit> {
  ALL.iter().find(|kit| kit.id == id).copied()
}

#[allow(dead_code)] // no production caller yet — exists for future use (e.g. a list_agents command) and this module's own tests
pub fn all() -> &'static [&'static AgentKit] {
  ALL
}

pub fn plans_dir(kit: &AgentKit) -> String {
  format!("{}/plans", kit.home_dir)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn known_agents_resolve() {
    assert_eq!(get("claude").map(|k| k.cli_token), Some("claude"));
    assert_eq!(get("codex").map(|k| k.cli_token), Some("codex"));
  }

  #[test]
  fn unknown_agent_returns_none() {
    assert!(get("gpt4").is_none());
  }

  #[test]
  fn every_agent_has_a_default_within_its_own_modes() {
    for kit in all() {
      assert!(
        kit.permission_modes.contains(&kit.default_permission_mode),
        "{}'s default_permission_mode isn't in its own permission_modes",
        kit.id
      );
    }
  }

  #[test]
  fn plans_dir_is_home_dir_slash_plans() {
    assert_eq!(plans_dir(&CLAUDE), "/home/agent/.claude/plans");
    assert_eq!(plans_dir(&CODEX), "/home/agent/.codex/plans");
  }
}
