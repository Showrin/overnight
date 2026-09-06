// Machine-wide preset, applied via `sbx policy init <preset>` (see
// commands.rs's VALID_NETWORK_POLICY_PRESETS). Distinct vocabulary from
// SANDBOX_NETWORK_PRESET_OVERRIDES below — sbx's own preset names vs. the
// app's own per-sandbox override terms — but labeled the same way in the
// UI since they represent the same user-facing idea at different scopes.
export const NETWORK_POLICY_PRESETS = ['allow-all', 'balanced', 'deny-all'] as const
export type NetworkPolicyPreset = (typeof NETWORK_POLICY_PRESETS)[number]

export const NETWORK_POLICY_PRESET_LABELS: Record<NetworkPolicyPreset, string> = {
  'allow-all': 'Open',
  balanced: 'Balanced',
  'deny-all': 'Locked Down',
}

// Per-sandbox override: only Open or Locked Down, never Balanced — sbx
// has no documented way to scope Balanced's whole rule set to a single
// sandbox, only a single wildcard allow/deny rule (see
// commands.rs::set_sandbox_network_preset_override).
export const SANDBOX_NETWORK_PRESET_OVERRIDES = ['open', 'locked-down'] as const
export type SandboxNetworkPresetOverride = (typeof SANDBOX_NETWORK_PRESET_OVERRIDES)[number]

export const SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS: Record<SandboxNetworkPresetOverride, string> = {
  open: 'Open',
  'locked-down': 'Locked Down',
}

export type NetworkRuleDecision = 'allow' | 'deny'

// Mirrors sbx/mod.rs's PolicyRule — always read live from `sbx policy ls
// --wide`, never persisted locally.
export interface PolicyRule {
  host: string
  decision: string
  source: string
}

export interface NetworkPolicySettings {
  preset: string | null
  rules: PolicyRule[]
}
