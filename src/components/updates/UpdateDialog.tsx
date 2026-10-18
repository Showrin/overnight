import { X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { useUpdater } from '@/store/useUpdater'
import { UpdateChangelog } from './UpdateChangelog'
import { UpdateProgress } from './UpdateProgress'

/** Full changelog of a pending update, opened from the update toast. */
export function UpdateDialog() {
  const open = useUpdater((s) => s.changelogOpen)
  const setOpen = useUpdater((s) => s.setChangelogOpen)
  const update = useUpdater((s) => s.update)
  const currentVersion = useUpdater((s) => s.currentVersion)
  const changelog = useUpdater((s) => s.changelog)
  const status = useUpdater((s) => s.status)
  const error = useUpdater((s) => s.error)
  const installUpdate = useUpdater((s) => s.installUpdate)

  const busy = status === 'downloading' || status === 'installing'

  return (
    <Dialog open={open && update !== null} onOpenChange={(next) => !busy && setOpen(next)}>
      <DialogContent
        title="What's new"
        className="flex h-[80vh] max-w-2xl flex-col overflow-hidden rounded-xl border border-border bg-background p-0"
      >
        {update && (
          <>
            <div className="flex h-14 shrink-0 items-center gap-4 border-b border-border pr-3 pl-6">
              <div className="flex flex-col">
                <h1 className="text-base font-medium">Overnight v{update.version}</h1>
                {currentVersion && <span className="text-xs text-muted-foreground">You have v{currentVersion}</span>}
              </div>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Close"
                className="ml-auto"
                disabled={busy}
                onClick={() => setOpen(false)}
              >
                <X className="size-4" strokeWidth={1.5} />
              </Button>
            </div>
            <div className="min-h-0 flex-1 overflow-auto px-6 py-5">
              <UpdateChangelog entries={changelog} />
            </div>
            <div className="flex shrink-0 flex-col gap-3 border-t border-border px-6 py-4">
              <UpdateProgress />
              {status === 'error' && error && <p className="text-xs text-destructive">{error}</p>}
              <div className="flex justify-end gap-2">
                <Button variant="outline" size="sm" disabled={busy} onClick={() => setOpen(false)}>
                  Later
                </Button>
                <Button size="sm" disabled={busy} onClick={installUpdate}>
                  {busy ? 'Updating…' : 'Update and restart'}
                </Button>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
