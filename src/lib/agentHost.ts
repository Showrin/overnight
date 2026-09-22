// Agent id -> what `sbx run --name <sandbox> <id>` launches. Add an entry
// here (and to the AGENTS tuple in src-tauri/src/agents/mod.rs) to support
// another agent.
export const AGENTS = ['claude', 'codex'] as const
export type Agent = (typeof AGENTS)[number]

export const AGENT_LABELS: Record<Agent, string> = {
  claude: 'Claude',
  codex: 'Codex',
}

// This agent's home directory inside the sandbox — mirrors
// src-tauri/src/agents/mod.rs's AgentKit::home_dir. Used for display only
// (e.g. the Plans tab's source-path label); the backend is the source of
// truth for anything that actually reads/writes these paths.
export const AGENT_HOME_DIRS: Record<Agent, string> = {
  claude: '/home/agent/.claude',
  codex: '/home/agent/.codex',
}
