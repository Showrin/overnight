import type { SandboxStatus } from '@/components/sandboxes/types'

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
