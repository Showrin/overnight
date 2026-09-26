import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, RotateCcw, X } from 'lucide-react'
import { BackupCard } from '@/components/backups/BackupCard'
import { RestoreDialog } from '@/components/backups/RestoreDialog'
import type { SandboxBackup } from '@/components/backups/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
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
  const restoreOp = useAppStore((s) =>
    s.activeOperations.find((op) => op.kind === 'restore' && op.sandbox_id === sandbox.id),
  )
  const cancelOperation = useAppStore((s) => s.cancelOperation)
  const isRestoring = restoreOp != null

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
      <SandboxBackupSettings sandbox={sandbox} />

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
        {restoreOp ? (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="size-3.5 animate-spin text-primary" />
            Restore in progress…
            <Button
              size="sm"
              variant="ghost"
              className="ml-auto"
              title="The sandbox may be left partly restored"
              onClick={() => cancelOperation(restoreOp)}
            >
              <X className="size-3.5" />
              Cancel
            </Button>
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

function SandboxBackupSettings({ sandbox }: { sandbox: Sandbox }) {
  const autoBackupEnabled = useAppStore((s) => s.autoBackupEnabled)
  const globalInterval = useAppStore((s) => s.backupIntervalMinutes)
  const [customInterval, setCustomInterval] = useState<number | null>(sandbox.backup_interval_minutes)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setCustomInterval(sandbox.backup_interval_minutes)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  async function save(enabled: boolean, intervalMinutes: number | null) {
    setError(null)
    try {
      await invoke('save_sandbox_backup_settings', { id: sandbox.id, enabled, intervalMinutes })
      await useAppStore.getState().loadSandboxes()
    } catch (e) {
      setError(String(e))
    }
  }

  const intervalDisabled = !autoBackupEnabled || !sandbox.backup_enabled

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-border p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex flex-col">
          <span className="text-xs font-medium text-foreground">Scheduled backups</span>
          <span className="text-xs text-muted-foreground">
            {autoBackupEnabled ? 'Back up this sandbox on a timer.' : 'Auto backup is off in Settings.'}
          </span>
        </div>
        <Switch
          checked={sandbox.backup_enabled}
          disabled={!autoBackupEnabled}
          onCheckedChange={(on) => save(on, sandbox.backup_interval_minutes)}
        />
      </div>

      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-muted-foreground">Custom interval (default {globalInterval} min)</span>
        <Switch
          checked={customInterval != null}
          disabled={intervalDisabled}
          onCheckedChange={(on) => {
            const next = on ? globalInterval : null
            setCustomInterval(next)
            save(sandbox.backup_enabled, next)
          }}
        />
      </div>

      {customInterval != null && (
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Input
            type="number"
            min={1}
            className="max-w-24"
            value={customInterval}
            disabled={intervalDisabled}
            onChange={(e) => setCustomInterval(Math.max(1, Number(e.target.value) || 1))}
            onBlur={() => {
              if (customInterval !== sandbox.backup_interval_minutes) save(sandbox.backup_enabled, customInterval)
            }}
          />
          minutes
        </div>
      )}

      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  )
}
