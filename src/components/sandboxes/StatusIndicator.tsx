import { Badge } from '@/components/ui/badge'
import { statusBadgeVariant } from '@/lib/sandboxDisplay'
import type { SandboxStatus } from './types'

export function StatusIndicator({ status }: { status: SandboxStatus }) {
  if (status === 'running') {
    return (
      <span
        role="status"
        aria-label="running"
        title="running"
        className="ml-1 size-2 shrink-0 rounded-full bg-success"
      />
    )
  }
  return <Badge variant={statusBadgeVariant(status)}>{status}</Badge>
}
