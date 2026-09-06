import type { SandboxStatus } from '@/components/sandboxes/types'
import {
  NETWORK_POLICY_PRESET_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS,
  type NetworkPolicyPreset,
  type SandboxNetworkPresetOverride,
} from '@/lib/networkPolicy'

type BadgeVariant = 'outline-muted' | 'outline-success' | 'outline-warning' | 'outline-destructive'

export function statusBadgeVariant(status: SandboxStatus): BadgeVariant {
  switch (status) {
    case 'running':
      return 'outline-success'
    case 'error':
      return 'outline-destructive'
    case 'starting':
    case 'stopping':
      return 'outline-warning'
    case 'stopped':
    default:
      return 'outline-muted'
  }
}

export function permissionBadgeVariant(permissionMode: string): BadgeVariant {
  if (permissionMode === 'bypassPermissions') return 'outline-destructive'
  if (permissionMode === 'acceptEdits') return 'outline-warning'
  return 'outline-muted'
}

// Formats a past unix-ms timestamp as "3 minutes ago"-style relative text,
// using the platform's own `Intl.RelativeTimeFormat` rather than a new
// dependency for one caption. Shared by SandboxCard's backup caption and
// the Branch tab's "as of <relative time>" freshness label.
export function formatRelativeTime(pastMs: number): string {
  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' })
  const divisions: [Intl.RelativeTimeFormatUnit, number][] = [
    ['second', 60],
    ['minute', 60],
    ['hour', 24],
    ['day', 30],
    ['month', 12],
    ['year', Infinity],
  ]
  let duration = (pastMs - Date.now()) / 1000
  for (const [unit, amount] of divisions) {
    if (Math.abs(duration) < amount) {
      return rtf.format(Math.round(duration), unit)
    }
    duration /= amount
  }
  return rtf.format(Math.round(duration), 'year')
}

// Formats a sandbox's effective network policy as a short label — either
// its own override, or "Global default (<preset>)" when it inherits one.
export function formatNetworkPolicyLabel(
  override: string | null,
  globalPreset: string | null
): string {
  if (override) {
    return SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS[override as SandboxNetworkPresetOverride] ?? override
  }
  if (globalPreset) {
    return `Global default (${NETWORK_POLICY_PRESET_LABELS[globalPreset as NetworkPolicyPreset] ?? globalPreset})`
  }
  return 'Global default'
}
