import type { Agent } from './agentHost'

// Mirrors src-tauri/src/agents/mod.rs's AgentKit::permission_modes. Each
// list is ordered least-restrictive-approval-required first, most-permissive
// (no approval ever asked) last — sandboxDisplay.ts's permissionTextClass
// and fullPermissionMode() below both rely on that ordering, not just the
// values. Codex's modes ('untrusted' | 'on-failure' | 'on-request' | 'never')
// are its --ask-for-approval values — unverified against a real Codex CLI
// install, see agents/mod.rs's doc comment on CODEX.
export const PERMISSION_MODES: Record<Agent, readonly string[]> = {
  claude: ['plan', 'default', 'acceptEdits', 'bypassPermissions'],
  codex: ['untrusted', 'on-failure', 'on-request', 'never'],
}

export type PermissionMode = string

// Mirrors src-tauri/src/agents/mod.rs's AgentKit::default_permission_mode —
// each agent's own baseline mode, used when switching the global default
// agent (Settings) resets the single shared permission-mode setting to a
// value that's actually valid for the newly selected agent.
export const DEFAULT_PERMISSION_MODE: Record<Agent, string> = {
  claude: 'default',
  codex: 'never',
}

// The most-permissive ("skip every approval") mode for an agent — always
// the last entry in PERMISSION_MODES[agent], per that list's documented
// ordering. Used when a user switches an in-progress sandbox creation to a
// different agent than the global default: rather than silently keeping a
// mode string that may not even exist for the new agent, they land on its
// most-permissive mode explicitly (still visible and changeable), not a
// smaller-permission default that could surprise them mid-task.
export function fullPermissionMode(agent: Agent): string {
  const modes = PERMISSION_MODES[agent]
  return modes[modes.length - 1]
}
