import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, RotateCcw } from 'lucide-react'
import { BackupCard } from '@/components/backups/BackupCard'
import { RestoreDialog } from '@/components/backups/RestoreDialog'
import type { SandboxBackup } from '@/components/backups/types'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

// No shared store, no polling — matches SandboxPlansTab's pattern: owns its
// own fetch, refreshed on mount and after any change this tab causes.
export function SandboxBackupsTab({ sandbox }: { sandbox: Sandbox }) {
  const [backups, setBackups] = useState<SandboxBackup[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [restoreOpen, setRestoreOpen] = useState(false)
  const isRestoring = useAppStore((s) =>
    s.activeOperations.some((op) => op.kind === 'restore' && op.sandbox_id === sandbox.id),
  )

  async function load() {
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
    load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  const ownBackups = backups.filter((b) => b.sandbox_id === sandbox.id)

  return (
    <div className="flex flex-col gap-4 text-sm">
      <div
        className={cn(
          'flex flex-col gap-2 rounded-lg border border-border p-3',
          isRestoring && 'ring-2 ring-primary/50 shadow-md shadow-primary/30 transition-shadow',
        )}
      >
        <span className="text-xs font-medium text-foreground">Restore from another sandbox</span>
        <p className="text-xs text-muted-foreground">
          Pick a backup from any sandbox and restore it into this sandbox.
        </p>
        {isRestoring ? (
          <div className="flex w-fit items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="size-3.5 animate-spin text-primary" />
            Restore in progress…
          </div>
        ) : (
          <Button size="sm" variant="outline" className="w-fit" onClick={() => setRestoreOpen(true)}>
            <RotateCcw className="size-3.5" />
            Restore…
          </Button>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <span className="text-xs font-medium text-foreground">Backups</span>
        {loading && <p className="text-sm text-muted-foreground">Loading…</p>}
        {error && <p className="text-sm text-destructive">{error}</p>}
        {!loading && !error && ownBackups.length === 0 && (
          <p className="text-sm text-muted-foreground">No backups yet.</p>
        )}
        {ownBackups.map((backup) => (
          <BackupCard key={backup.id} backup={backup} allBackups={backups} onDeleted={load} />
        ))}
      </div>

      <RestoreDialog open={restoreOpen} onOpenChange={setRestoreOpen} allBackups={backups} fixedTargetSandboxId={sandbox.id} />
    </div>
  )
}
