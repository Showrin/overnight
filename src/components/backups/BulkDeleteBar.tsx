import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'

export function BulkDeleteBar({
  selected,
  allIds,
  onSelectAll,
  onCancel,
  onDeleted,
}: {
  selected: Set<string>
  allIds: string[]
  onSelectAll: (checked: boolean) => void
  onCancel: () => void
  onDeleted: () => void
}) {
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const count = selected.size
  const allSelected = allIds.length > 0 && allIds.every((id) => selected.has(id))
  const noun = count === 1 ? 'backup' : 'backups'

  async function handleDelete() {
    setDeleting(true)
    setError(null)
    try {
      await invoke<number>('delete_sandbox_backups', { ids: Array.from(selected) })
      setConfirmOpen(false)
      onDeleted()
    } catch (e) {
      setError(String(e))
    } finally {
      setDeleting(false)
    }
  }

  return (
    <div className="sticky top-0 z-10 flex items-center gap-2 rounded-lg border border-border bg-background p-2">
      <span className="text-sm">{count} selected</span>
      <Button size="sm" variant="ghost" onClick={() => onSelectAll(!allSelected)} disabled={allIds.length === 0}>
        {allSelected ? 'Clear all' : 'Select all'}
      </Button>
      <div className="ml-auto flex items-center gap-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button size="sm" variant="destructive" disabled={count === 0} onClick={() => setConfirmOpen(true)}>
          <Trash2 className="size-3.5" />
          Delete selected
        </Button>
      </div>

      <Dialog open={confirmOpen} onOpenChange={(open) => !deleting && setConfirmOpen(open)}>
        <DialogContent title={`Delete ${count} ${noun}?`}>
          <Card className="w-full">
            <CardHeader>
              <CardTitle>
                Delete {count} {noun}?
              </CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                This permanently deletes {count} selected {noun}. This can't be undone.
              </p>
              {error && <p className="text-sm text-destructive">{error}</p>}
              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => setConfirmOpen(false)} disabled={deleting}>
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
    </div>
  )
}
