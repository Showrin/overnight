import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { FolderOpen, Loader2, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { formatRelativeTime } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import { BackupCard, TRIGGER_LABELS } from './BackupCard'
import type { SandboxBackup } from './types'

const COLLAPSED_COUNT = 2

export function BackupsScreen() {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const activeBackups = useAppStore((s) => s.activeBackups)

  const [backups, setBackups] = useState<SandboxBackup[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set())
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null)
  const [deletingGroup, setDeletingGroup] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)

  async function loadBackups() {
    setLoading(true)
    setError(null)
    try {
      const result = await invoke<SandboxBackup[]>('list_backups')
      setBackups(result)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    loadBackups()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // A completed (or newly started) backup changes the active count — reload
  // the list so a just-finished backup shows up without a manual refresh.
  useEffect(() => {
    if (!loading) loadBackups()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeBackups.length])

  // Grouped regardless of whether the sandbox still exists — backups are no
  // longer cascade-deleted with their sandbox, so a deleted sandbox's
  // history stays visible here (see sandbox_label fallback below).
  const groups = new Map<string, SandboxBackup[]>()
  for (const backup of backups) {
    const group = groups.get(backup.sandbox_id)
    if (group) group.push(backup)
    else groups.set(backup.sandbox_id, [backup])
  }

  function groupLabel(sandboxId: string, sandboxBackups: SandboxBackup[]) {
    const sandbox = sandboxes.find((s) => s.id === sandboxId)
    return sandbox?.name ?? sandbox?.sbx_name ?? sandboxBackups[0]?.sandbox_label ?? sandboxId
  }

  async function handleConfirmDelete() {
    if (!confirmDeleteId) return
    setDeletingGroup(true)
    setDeleteError(null)
    try {
      await invoke('delete_sandbox_backups_for_sandbox', { sandboxId: confirmDeleteId })
      setConfirmDeleteId(null)
      await loadBackups()
    } catch (e) {
      setDeleteError(String(e))
    } finally {
      setDeletingGroup(false)
    }
  }

  return (
    <div className="flex w-full flex-col gap-4">
      <div className="flex items-center justify-between gap-2">
        <h1 className="text-lg font-medium">Backups</h1>
        <Button
          size="sm"
          variant="outline"
          onClick={() => invoke('open_backups_root_folder')}
          title="Open the folder every sandbox's backups are stored under"
        >
          <FolderOpen className="size-3.5" />
          Open Folder
        </Button>
      </div>

      {activeBackups.map((active) => {
        const sandbox = sandboxes.find((s) => s.id === active.sandbox_id)
        return (
          <Card key={active.sandbox_id} className="w-full border-primary/40">
            <CardContent className="flex items-center gap-3">
              <Loader2 className="size-4 shrink-0 animate-spin text-primary" />
              <div className="flex flex-col">
                <span className="text-sm font-medium">
                  Backing up {sandbox?.name ?? sandbox?.sbx_name ?? active.sandbox_id}
                </span>
                <span className="text-xs text-muted-foreground">
                  {TRIGGER_LABELS[active.trigger] ?? active.trigger} — started {formatRelativeTime(active.started_at)}
                </span>
              </div>
            </CardContent>
          </Card>
        )
      })}

      {loading && <p className="text-sm text-muted-foreground">Loading…</p>}
      {error && <p className="text-sm text-destructive">{error}</p>}
      {!loading && !error && groups.size === 0 && activeBackups.length === 0 && (
        <p className="text-sm text-muted-foreground">No backups yet.</p>
      )}

      {Array.from(groups.entries()).map(([sandboxId, sandboxBackups]) => {
        const expanded = expandedGroups.has(sandboxId)
        const visible = expanded ? sandboxBackups : sandboxBackups.slice(0, COLLAPSED_COUNT)
        const hasMore = sandboxBackups.length > COLLAPSED_COUNT
        return (
          <div key={sandboxId} className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm font-medium text-foreground">{groupLabel(sandboxId, sandboxBackups)}</span>
              <Button
                size="icon-sm"
                variant="ghost"
                title="Delete all backups for this sandbox"
                onClick={() => setConfirmDeleteId(sandboxId)}
              >
                <Trash2 className="size-3.5" />
              </Button>
            </div>
            {visible.map((backup) => (
              <BackupCard key={backup.id} backup={backup} allBackups={backups} onDeleted={loadBackups} />
            ))}
            {hasMore && (
              <Button
                size="sm"
                variant="ghost"
                className="w-fit"
                onClick={() =>
                  setExpandedGroups((prev) => {
                    const next = new Set(prev)
                    if (expanded) next.delete(sandboxId)
                    else next.add(sandboxId)
                    return next
                  })
                }
              >
                {expanded ? 'See less' : `See more (${sandboxBackups.length - COLLAPSED_COUNT})`}
              </Button>
            )}
          </div>
        )
      })}

      <Dialog open={confirmDeleteId != null} onOpenChange={(open) => !open && setConfirmDeleteId(null)}>
        <DialogContent title="Delete all backups?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Delete all backups?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                This permanently deletes every backup of{' '}
                {confirmDeleteId ? groupLabel(confirmDeleteId, groups.get(confirmDeleteId) ?? []) : 'this sandbox'}. This
                can't be undone.
              </p>
              {deleteError && <p className="text-sm text-destructive">{deleteError}</p>}
              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => setConfirmDeleteId(null)} disabled={deletingGroup}>
                  Cancel
                </Button>
                <Button variant="destructive" onClick={handleConfirmDelete} disabled={deletingGroup}>
                  {deletingGroup && <Loader2 className="size-3.5 animate-spin" />}
                  Delete all
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </div>
  )
}
