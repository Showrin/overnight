import { useEffect, useState } from 'react'
import { Check, Copy, X } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { branchSyncBadgeVariant, branchSyncShortLabel, branchSyncStatusLabel } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'

const AUTO_DISMISS_MS = 45000

function CopyBranchButton({ branch }: { branch: string }) {
  const [copied, setCopied] = useState(false)

  async function copy() {
    await navigator.clipboard.writeText(branch)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <button
      type="button"
      onClick={copy}
      aria-label={`Copy ${branch}`}
      title="Copy branch name"
      className="shrink-0 text-muted-foreground hover:text-foreground"
    >
      {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
    </button>
  )
}

// Small bottom-right popout showing the last Git Sync's per-branch output —
// replaces putting that list in the OS notification body, which platforms
// like Windows truncate. Auto-dismisses so it never becomes a fixture.
export function GitSyncToast() {
  const toast = useAppStore((s) => s.gitSyncToast)
  const dismiss = useAppStore((s) => s.dismissGitSyncToast)
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === toast?.sandboxId))
  const projectName = useAppStore((s) => s.projects.find((p) => p.id === sandbox?.project_id)?.name)

  useEffect(() => {
    if (!toast) return
    const timer = setTimeout(dismiss, AUTO_DISMISS_MS)
    return () => clearTimeout(timer)
  }, [toast, dismiss])

  if (!toast) return null

  const title = sandbox?.name ?? sandbox?.sbx_name ?? projectName ?? 'Sandbox'

  return (
    <div className="flex w-[336px] flex-col gap-2 overflow-hidden rounded-lg border border-border bg-card p-3 shadow-md">
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 flex-col">
          <span className="truncate text-sm font-medium text-foreground" title={title}>
            {title}
          </span>
          <span className="truncate text-xs text-muted-foreground">
            {toast.label}
            {projectName && title !== projectName && ` · ${projectName}`}
          </span>
        </div>
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
        <ul className="flex max-h-40 flex-col gap-1 overflow-x-hidden overflow-y-auto">
          {toast.outcomes.map((o) => (
            <li key={o.branch} className="flex min-w-0 items-center gap-2 text-xs">
              <CopyBranchButton branch={o.branch} />
              <span className="min-w-0 flex-1 truncate font-mono text-foreground" title={o.branch}>
                {o.branch}
              </span>
              <Badge variant={branchSyncBadgeVariant(o.status)} className="shrink-0" title={branchSyncStatusLabel(o.status)}>
                {branchSyncShortLabel(o.status)}
              </Badge>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
