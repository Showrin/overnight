// Agent id -> what `sbx run --name <sandbox> <id>` launches. Add an entry
// here (and to the AGENTS tuple in src-tauri/src/commands.rs) to support
// another agent.
export const AGENTS = ['claude'] as const
export type Agent = (typeof AGENTS)[number]

export const AGENT_LABELS: Record<Agent, string> = {
  claude: 'Claude',
}
