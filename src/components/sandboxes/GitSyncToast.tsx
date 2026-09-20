import { useEffect } from 'react'
import { X } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { branchSyncBadgeVariant, branchSyncStatusLabel } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'

const AUTO_DISMISS_MS = 45000

// Small bottom-right popout showing the last Git Sync's per-branch output —
// replaces putting that list in the OS notification body, which platforms
// like Windows truncate. Auto-dismisses so it never becomes a fixture.
export function GitSyncToast() {
  const toast = useAppStore((s) => s.gitSyncToast)
  const dismiss = useAppStore((s) => s.dismissGitSyncToast)

  useEffect(() => {
    if (!toast) return
    const timer = setTimeout(dismiss, AUTO_DISMISS_MS)
    return () => clearTimeout(timer)
  }, [toast, dismiss])

  if (!toast) return null

  return (
    <div className="fixed right-4 bottom-4 z-50 flex w-72 flex-col gap-2 rounded-lg border border-border bg-card p-3 shadow-md">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-medium text-foreground">{toast.label}</span>
        <button
          type="button"
          onClick={dismiss}
          aria-label="Dismiss"
          className="shrink-0 text-muted-foreground hover:text-foreground"
        >
          <X className="size-3.5" />
        </button>
      </div>
      {toast.outcomes.length === 0 ? (
        <p className="text-xs text-muted-foreground">No sandbox branches to sync.</p>
      ) : (
        <ul className="flex max-h-40 flex-col gap-1 overflow-y-auto">
          {toast.outcomes.map((o) => (
            <li key={o.branch} className="flex items-center justify-between gap-2 text-xs">
              <span className="truncate font-mono text-foreground">{o.branch}</span>
              <Badge variant={branchSyncBadgeVariant(o.status)}>{branchSyncStatusLabel(o.status)}</Badge>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
