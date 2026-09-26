import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { notify } from '@/lib/notify'
import { useAppStore } from '@/store/useAppStore'
import type { BackupScope, SandboxBackup } from './types'

function scopeLabel(scope: BackupScope, backup: SandboxBackup | undefined): string {
  switch (scope) {
    case 'all': {
      const agentLabel = backup?.has_codex ? '.codex' : '.claude'
      return `All (${agentLabel} + .git)`
    }
    case 'claude':
      return '.claude only'
    case 'codex':
      return '.codex only'
    case 'git':
      return '.git only'
  }
}

function availableScopes(backup: SandboxBackup | undefined): BackupScope[] {
  if (!backup) return []
  const scopes: BackupScope[] = []
  const hasAgentData = backup.has_claude || backup.has_codex
  if (hasAgentData && backup.has_git) scopes.push('all')
  if (backup.has_claude) scopes.push('claude')
  if (backup.has_codex) scopes.push('codex')
  if (backup.has_git) scopes.push('git')
  return scopes
}

export function RestoreDialog({
  open,
  onOpenChange,
  allBackups,
  fixedBackup,
  fixedTargetSandboxId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  allBackups: SandboxBackup[]
  fixedBackup?: SandboxBackup
  fixedTargetSandboxId?: string
}) {
  const sandboxes = useAppStore((s) => s.sandboxes)
  const runningSandboxes = sandboxes.filter((s) => s.status === 'running')

  const [sourceSandboxId, setSourceSandboxId] = useState('')
  const [selectedBackupId, setSelectedBackupId] = useState('')
  const [targetSandboxId, setTargetSandboxId] = useState('')
  const [scope, setScope] = useState<BackupScope>(() => availableScopes(fixedBackup)[0] ?? 'all')

  useEffect(() => {
    if (open) {
      setSourceSandboxId('')
      setSelectedBackupId('')
      setTargetSandboxId('')
    }
  }, [open])

  const sourceSandboxIds = Array.from(new Set(allBackups.map((b) => b.sandbox_id)))
  const sourceBackups = allBackups.filter((b) => b.sandbox_id === sourceSandboxId)
  const effectiveBackup = fixedBackup ?? allBackups.find((b) => b.id === selectedBackupId)
  const scopes = availableScopes(effectiveBackup)

  // Re-validates every time the dialog opens, not just when the backup
  // identity changes — reopening the same card's dialog doesn't change
  // effectiveBackup.id, but a leftover scope from a previous open (e.g.
  // "all" left selected from a full backup) can still be invalid for this
  // one (a .git-only backup has no "all"), which silently disabled Restore.
  useEffect(() => {
    if (!open) return
    if (scopes.length > 0 && !scopes.includes(scope)) {
      setScope(scopes[0])
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [effectiveBackup?.id, open])

  const targetId = fixedTargetSandboxId ?? targetSandboxId
  const canConfirm = effectiveBackup != null && targetId !== '' && scopes.includes(scope)

  function handleConfirm() {
    if (!canConfirm || !effectiveBackup) return
    onOpenChange(false)
    invoke('restore_backup', { backupId: effectiveBackup.id, targetSandboxId: targetId, scope })
      .then(() => notify('Restore complete'))
      .catch((e) => {
        // notify() depends on OS notification permission being granted —
        // don't let a denied/unset permission make a failure invisible.
        console.error('restore_backup failed:', e)
        const message = String(e)
        notify(message.startsWith('restore cancelled') ? 'Restore cancelled' : 'Restore failed', message)
      })
      .finally(() => {
        useAppStore.getState().loadActiveOperations()
      })
    // The 5s poll can easily miss a restore entirely — a small .claude
    // restore can start and finish inside one poll gap, so both the
    // global indicator and the per-sandbox progress row would never see
    // it as active. Refresh right away instead of waiting for the poll;
    // a second refresh shortly after covers the case where this first one
    // raced ahead of the backend registering the operation as active.
    useAppStore.getState().loadActiveOperations()
    setTimeout(() => useAppStore.getState().loadActiveOperations(), 300)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent title="Restore backup" onPointerDownOutside={(e) => e.preventDefault()}>
        <Card className="w-full">
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle>Restore backup</CardTitle>
            <Button size="icon-sm" variant="ghost" onClick={() => onOpenChange(false)} title="Close">
              <X className="size-3.5" />
            </Button>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            {!fixedBackup && (
              <div className="flex flex-col gap-2">
                <div className="flex flex-col gap-1">
                  <span className="text-xs text-muted-foreground/70">From sandbox</span>
                  <Select
                    value={sourceSandboxId}
                    onValueChange={(value) => {
                      setSourceSandboxId(value)
                      setSelectedBackupId('')
                    }}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Select a sandbox" />
                    </SelectTrigger>
                    <SelectContent>
                      {sourceSandboxIds.map((id) => {
                        const sandbox = sandboxes.find((s) => s.id === id)
                        return (
                          <SelectItem key={id} value={id}>
                            {sandbox?.name ?? sandbox?.sbx_name ?? id}
                          </SelectItem>
                        )
                      })}
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex flex-col gap-1">
                  <span className="text-xs text-muted-foreground/70">Backup</span>
                  <Select value={selectedBackupId} onValueChange={setSelectedBackupId} disabled={!sourceSandboxId}>
                    <SelectTrigger>
                      <SelectValue placeholder={sourceSandboxId ? 'Select a backup' : 'Select a sandbox first'} />
                    </SelectTrigger>
                    <SelectContent>
                      {sourceBackups.map((b) => (
                        <SelectItem key={b.id} value={b.id}>
                          {new Date(b.created_at).toLocaleString()}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>
            )}

            {!fixedTargetSandboxId && (
              <div className="flex flex-col gap-1">
                <span className="text-xs text-muted-foreground/70">Into sandbox</span>
                <Select value={targetSandboxId} onValueChange={setTargetSandboxId}>
                  <SelectTrigger>
                    <SelectValue placeholder="Select a sandbox" />
                  </SelectTrigger>
                  <SelectContent>
                    {runningSandboxes.map((sandbox) => (
                      <SelectItem key={sandbox.id} value={sandbox.id}>
                        {sandbox.name ?? sandbox.sbx_name ?? sandbox.id}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            <div className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground/70">What to restore</span>
              <Select value={scope} onValueChange={(value) => setScope(value as BackupScope)} disabled={scopes.length === 0}>
                <SelectTrigger>
                  <SelectValue placeholder="Select a backup first" />
                </SelectTrigger>
                <SelectContent>
                  {scopes.map((s) => (
                    <SelectItem key={s} value={s}>
                      {scopeLabel(s, effectiveBackup)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                Cancel
              </Button>
              <Button disabled={!canConfirm} onClick={handleConfirm}>
                Restore
              </Button>
            </div>
          </CardContent>
        </Card>
      </DialogContent>
    </Dialog>
  )
}
