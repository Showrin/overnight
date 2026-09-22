import type { Agent } from './agentHost'

// Mirrors src-tauri/src/agents/mod.rs's AgentKit::permission_modes.
// Codex's modes ('untrusted' | 'on-failure' | 'on-request' | 'never') are
// its --ask-for-approval values — unverified against a real Codex CLI
// install, see agents/mod.rs's doc comment on CODEX.
export const PERMISSION_MODES: Record<Agent, readonly string[]> = {
  claude: ['plan', 'default', 'acceptEdits', 'bypassPermissions'],
  codex: ['untrusted', 'on-failure', 'on-request', 'never'],
}

export type PermissionMode = string
