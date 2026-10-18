import { Download, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useUpdater } from '@/store/useUpdater'
import { UpdateProgress } from './UpdateProgress'

// Bottom-right popout announcing a new release. Dismissing it only hides it
// until a newer version shows up; the Updates settings section still offers
// the update.
export function UpdateToast() {
  const update = useUpdater((s) => s.update)
  const status = useUpdater((s) => s.status)
  const dismissed = useUpdater((s) => s.toastDismissed)
  const dismiss = useUpdater((s) => s.dismissToast)
  const setChangelogOpen = useUpdater((s) => s.setChangelogOpen)
  const installUpdate = useUpdater((s) => s.installUpdate)

  if (!update || dismissed) return null

  const busy = status === 'downloading' || status === 'installing'

  return (
    <div className="flex w-[336px] flex-col gap-2 rounded-lg border border-border bg-card p-3 shadow-md">
      <div className="flex items-start gap-2">
        <Download className="mt-0.5 size-3.5 shrink-0 text-primary" />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="text-sm font-medium text-foreground">Update available</span>
          <span className="text-xs text-muted-foreground">Overnight v{update.version} is ready to install.</span>
        </div>
        {!busy && (
          <button
            type="button"
            onClick={dismiss}
            aria-label="Dismiss"
            className="shrink-0 text-muted-foreground hover:text-foreground"
          >
            <X className="size-3.5" />
          </button>
        )}
      </div>
      <UpdateProgress />
      {status === 'error' && <p className="text-xs text-destructive">Update failed. Try again from Settings → Updates.</p>}
      {!busy && (
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="xs" onClick={() => setChangelogOpen(true)}>
            What's new
          </Button>
          <Button size="xs" onClick={installUpdate}>
            Update now
          </Button>
        </div>
      )}
    </div>
  )
}
