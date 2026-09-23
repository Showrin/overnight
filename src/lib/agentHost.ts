// Agent id -> what `sbx run --name <sandbox> <id>` launches. Add an entry
// here (and to the `ALL` slice in src-tauri/src/agents/mod.rs) to support
// another agent. `commands::list_agents` exposes that Rust registry over
// the wire, but these constants stay the frontend's actual source of
// truth for now — `Agent` below is a compile-time-checked string union,
// which a value loaded at runtime can't provide.
export const AGENTS = ['claude', 'codex'] as const
export type Agent = (typeof AGENTS)[number]

export const AGENT_LABELS: Record<Agent, string> = {
  claude: 'Claude',
  codex: 'Codex',
}

// Brand-colored border/text/background for the agent-launch button — the
// color tokens themselves live in src/index.css (--agent-claude/--agent-codex).
export const AGENT_ACCENT_CLASSES: Record<Agent, string> = {
  claude: 'border-agent-claude text-agent-claude bg-agent-claude/10 hover:bg-agent-claude/20 hover:text-agent-claude',
  codex: 'border-agent-codex text-agent-codex bg-agent-codex/10 hover:bg-agent-codex/20 hover:text-agent-codex',
}

// This agent's home directory inside the sandbox — mirrors
// src-tauri/src/agents/mod.rs's AgentKit::home_dir. Used for display only
// (e.g. the Plans tab's source-path label); the backend is the source of
// truth for anything that actually reads/writes these paths.
export const AGENT_HOME_DIRS: Record<Agent, string> = {
  claude: '/home/agent/.claude',
  codex: '/home/agent/.codex',
}
