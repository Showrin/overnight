import { Loader2 } from 'lucide-react'
import type { Route } from '@/lib/router'
import { formatRelativeTime } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'

// Always-mounted, non-blocking progress panel for backups and restores in
// flight app-wide — no toast/notification primitive exists in this
// codebase, so this is a plain fixed-position panel, empty-render when
// there's nothing active.
export function OperationsIndicator({ navigate }: { navigate: (route: Route) => void }) {
  const operations = useAppStore((s) => s.activeOperations)
  const sandboxes = useAppStore((s) => s.sandboxes)

  if (operations.length === 0) return null

  function sandboxLabel(id: string) {
    const sandbox = sandboxes.find((s) => s.id === id)
    return sandbox?.name ?? sandbox?.sbx_name ?? id
  }

  return (
    <div className="fixed right-4 bottom-4 z-40 flex flex-col gap-2">
      {operations.map((op, index) => (
        <button
          key={`${op.kind}-${op.sandbox_id}-${op.started_at}-${index}`}
          type="button"
          onClick={() => navigate({ screen: 'sandboxes', sandboxId: op.sandbox_id, detailTab: 'backups' })}
          className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2 text-left text-sm shadow-md hover:bg-accent"
        >
          <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
          <div className="flex flex-col">
            <span className="font-medium text-foreground">
              {op.kind === 'backup'
                ? `Backing up ${sandboxLabel(op.sandbox_id)}`
                : `Restoring ${op.scope ?? 'all'} into ${sandboxLabel(op.sandbox_id)}${
                    op.source_sandbox_id ? ` from ${sandboxLabel(op.source_sandbox_id)}` : ''
                  }`}
            </span>
            <span className="text-xs text-muted-foreground">started {formatRelativeTime(op.started_at)}</span>
          </div>
        </button>
      ))}
    </div>
  )
}
