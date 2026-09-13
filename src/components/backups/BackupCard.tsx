import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { FolderOpen, Loader2, RotateCcw, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { formatBytes, formatRelativeTime } from '@/lib/sandboxDisplay'
import { RestoreDialog } from './RestoreDialog'
import type { SandboxBackup } from './types'

export const TRIGGER_LABELS: Record<string, string> = {
  scheduled: 'Scheduled',
  manual: 'Manual',
  pre_stop: 'Before stop',
  pre_delete: 'Before delete',
}

export function BackupCard({
  backup,
  allBackups,
  onDeleted,
}: {
  backup: SandboxBackup
  allBackups: SandboxBackup[]
  onDeleted: () => void
}) {
  const [deleting, setDeleting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [restoreOpen, setRestoreOpen] = useState(false)
  const [confirmDeleteOpen, setConfirmDeleteOpen] = useState(false)

  function openFolder() {
    invoke('open_path_in_explorer', { path: backup.host_dir })
  }

  async function handleDelete() {
    setDeleting(true)
    setError(null)
    try {
      await invoke('delete_sandbox_backup', { id: backup.id })
      setConfirmDeleteOpen(false)
      onDeleted()
    } catch (e) {
      setError(String(e))
      setDeleting(false)
    }
  }

  return (
    <Card className="w-full">
      <CardContent className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Badge variant="outline-muted">{TRIGGER_LABELS[backup.trigger] ?? backup.trigger}</Badge>
            <span className="text-xs text-muted-foreground">{formatRelativeTime(backup.created_at)}</span>
          </div>
          <div className="flex items-center gap-1">
            <Button size="icon-sm" variant="ghost" onClick={openFolder} title="Open backup folder">
              <FolderOpen className="size-3.5" />
            </Button>
            <Button
              size="icon-sm"
              variant="ghost"
              title="Choose a sandbox to restore this backup into"
              onClick={() => setRestoreOpen(true)}
            >
              <RotateCcw className="size-3.5" />
            </Button>
            <Button
              size="icon-sm"
              variant="ghost"
              disabled={deleting}
              onClick={() => setConfirmDeleteOpen(true)}
              title="Delete backup"
            >
              {deleting ? <Loader2 className="size-3.5 animate-spin" /> : <Trash2 className="size-3.5" />}
            </Button>
          </div>
        </div>
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <span>
            Base branch: {backup.base_branch ?? '—'} → {backup.current_branch ?? '—'}
          </span>
          <span>{backup.branches.length} branch(es)</span>
          <span>{backup.plan_file_count} plan file(s)</span>
          <span>{formatBytes(backup.size_bytes)}</span>
          <span>.claude {backup.has_claude ? 'included' : 'missing'}</span>
          <span>.git {backup.has_git ? 'included' : 'missing'}</span>
        </div>
      </CardContent>

      <RestoreDialog open={restoreOpen} onOpenChange={setRestoreOpen} allBackups={allBackups} fixedBackup={backup} />

      <Dialog open={confirmDeleteOpen} onOpenChange={(open) => !deleting && setConfirmDeleteOpen(open)}>
        <DialogContent title="Delete backup?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Delete backup?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                This permanently deletes this {TRIGGER_LABELS[backup.trigger]?.toLowerCase() ?? backup.trigger} backup
                from {formatRelativeTime(backup.created_at)}. This can't be undone.
              </p>
              {error && <p className="text-sm text-destructive">{error}</p>}
              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => setConfirmDeleteOpen(false)} disabled={deleting}>
                  Cancel
                </Button>
                <Button variant="destructive" onClick={handleDelete} disabled={deleting}>
                  {deleting && <Loader2 className="size-3.5 animate-spin" />}
                  Delete
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </Card>
  )
}
