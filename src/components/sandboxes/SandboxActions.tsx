import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, ChevronDown, Code, DatabaseBackup, ExternalLink, GitBranch, Loader2, Play, Square, TerminalSquare, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { notify } from '@/lib/notify'
import { AGENT_LABELS, type Agent } from '@/lib/agentHost'
import { TERMINAL_HOSTS, TERMINAL_HOST_LABELS, TERMINAL_HOST_OPEN_LABELS, type TerminalHost } from '@/lib/terminalHost'
import { useAppStore } from '@/store/useAppStore'
import type { BranchSyncOutcome, Sandbox } from './types'

function backupScopesFor(agent: string): { value: 'all' | 'claude' | 'codex' | 'git'; label: string }[] {
  const agentScope = agent === 'codex' ? 'codex' : 'claude'
  return [
    { value: 'all', label: 'Backup all' },
    { value: agentScope, label: `Backup .${agentScope}` },
    { value: 'git', label: 'Backup .git' },
  ]
}

type BusyAction = 'start' | 'stop' | 'delete' | 'vscode' | 'terminal' | 'agent' | 'git-sync' | 'backup' | null
type ConfirmAction = 'stop' | 'delete' | null

export function SandboxActions({
  sandbox,
  projectName,
  onChanged,
  context,
}: {
  sandbox: Sandbox
  projectName: string
  onChanged: () => void
  context: 'card' | 'detail'
}) {
  const defaultTerminalHost = useAppStore((s) => s.defaultTerminalHost)
  const saveDefaultTerminalHost = useAppStore((s) => s.saveDefaultTerminalHost)
  const platform = useAppStore((s) => s.platform)
  const isBackingUp = useAppStore((s) => s.activeBackups.some((b) => b.sandbox_id === sandbox.id))
  const showGitSyncToast = useAppStore((s) => s.showGitSyncToast)
  const [busyAction, setBusyAction] = useState<BusyAction>(null)
  const [error, setError] = useState<string | null>(null)
  const [terminalMenuOpen, setTerminalMenuOpen] = useState(false)
  const [backupMenuOpen, setBackupMenuOpen] = useState(false)
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>(null)
  const [notFoundDialogOpen, setNotFoundDialogOpen] = useState(false)

  async function run(which: Exclude<BusyAction, null>, action: () => Promise<unknown>) {
    setBusyAction(which)
    setError(null)
    try {
      await action()
      const title = sandbox.name ?? projectName
      if (which === 'start') notify('Sandbox started', `${title} is up and running.`, sandbox.id)
      if (which === 'stop') notify('Sandbox stopped', `${title} has been stopped.`, sandbox.id)
      if (which === 'delete') notify('Sandbox deleted', `${title} has been deleted.`, sandbox.id)
      onChanged()
    } catch (e) {
      const message = String(e)
      setError(message)
      if (which === 'delete' && message.includes('sandbox not found')) {
        setNotFoundDialogOpen(true)
      }
    } finally {
      setBusyAction(null)
    }
  }

  async function attemptDelete(force: boolean) {
    await run('delete', () => invoke('delete_sandbox', { id: sandbox.id, force }))
  }

  async function retryDeleteAsForce() {
    setNotFoundDialogOpen(false)
    await attemptDelete(true)
  }

  async function handleBackupChoice(withBackup: boolean) {
    const action = confirmAction
    if (!action) return
    setConfirmAction(null)
    if (withBackup) {
      setBusyAction(action)
      setError(null)
      try {
        await invoke('backup_sandbox_now', { id: sandbox.id, trigger: action === 'stop' ? 'pre_stop' : 'pre_delete', scope: 'all' })
      } catch (e) {
        setError(String(e))
        setBusyAction(null)
        return
      }
    }
    if (action === 'delete') {
      await attemptDelete(false)
    } else {
      await run(action, () => invoke('stop_sandbox', { id: sandbox.id }))
    }
  }

  function openInBrowser() {
    if (sandbox.host_port != null) {
      window.open(`http://localhost:${sandbox.host_port}`, '_blank')
    }
  }

  async function handleBackupNow(scope: 'all' | 'claude' | 'codex' | 'git') {
    setBusyAction('backup')
    setError(null)
    try {
      await invoke('backup_sandbox_now', { id: sandbox.id, trigger: 'manual', scope })
      onChanged()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusyAction(null)
    }
  }

  async function gitSync() {
    setBusyAction('git-sync')
    setError(null)
    try {
      const outcomes = await invoke<BranchSyncOutcome[]>('git_sync_sandbox', { id: sandbox.id })
      const fastForwarded = outcomes.filter((o) => o.status === 'fast_forwarded').length
      const needsManualMerge = outcomes.filter((o) => o.status === 'needs_manual_merge').length
      const summary = needsManualMerge > 0
        ? `${fastForwarded} of ${outcomes.length} branch(es) fast-forwarded — ${needsManualMerge} need${needsManualMerge === 1 ? '' : 's'} manual merge.`
        : `${fastForwarded} of ${outcomes.length} branch(es) fast-forwarded.`
      notify('Git Sync complete', summary, sandbox.id, true)
      showGitSyncToast({ sandboxId: sandbox.id, label: `Git Sync — ${sandbox.name ?? projectName}`, outcomes })
      onChanged()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusyAction(null)
    }
  }

  const backupScopes = backupScopesFor(sandbox.agent)

  return (
    <div className="flex flex-col gap-6">
      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="flex flex-wrap gap-2">
        {sandbox.status === "running" && (
          <>
            <Button
              size="sm"
              variant="outline"
              disabled={busyAction != null}
              onClick={() =>
                run("vscode", () =>
                  invoke("open_sandbox_vscode", { id: sandbox.id }),
                )
              }
            >
              {busyAction === "vscode" ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Code className="size-3.5" />
              )}
              VS Code
            </Button>
            {platform === "windows" ? (
              context === "detail" && (
                <div className="inline-flex">
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busyAction != null}
                    className="rounded-r-none border-r-0"
                    onClick={() =>
                      run("terminal", () =>
                        invoke("open_sandbox_terminal", {
                          id: sandbox.id,
                          terminalHost: defaultTerminalHost,
                        }),
                      )
                    }
                  >
                    {busyAction === "terminal" ? (
                      <Loader2 className="size-3.5 animate-spin" />
                    ) : (
                      <TerminalSquare className="size-3.5" />
                    )}
                    {TERMINAL_HOST_OPEN_LABELS[defaultTerminalHost as TerminalHost] ?? "Open terminal"}
                  </Button>
                  <Popover open={terminalMenuOpen} onOpenChange={setTerminalMenuOpen}>
                    <PopoverTrigger asChild>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={busyAction != null}
                        className="rounded-l-none px-1"
                        aria-label="Choose terminal app"
                      >
                        <ChevronDown className="size-3.5" />
                      </Button>
                    </PopoverTrigger>
                    <PopoverContent align="end" className="flex flex-col gap-0.5">
                      {TERMINAL_HOSTS.map((host) => (
                        <button
                          key={host}
                          type="button"
                          onClick={() => {
                            setTerminalMenuOpen(false)
                            saveDefaultTerminalHost(host)
                          }}
                          className="flex w-full items-center justify-between gap-4 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground"
                        >
                          {TERMINAL_HOST_LABELS[host]}
                          {host === defaultTerminalHost && <Check className="size-3.5" />}
                        </button>
                      ))}
                    </PopoverContent>
                  </Popover>
                </div>
              )
            ) : (
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() =>
                  run("terminal", () => invoke("open_sandbox_terminal", { id: sandbox.id }))
                }
              >
                {busyAction === "terminal" ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <TerminalSquare className="size-3.5" />
                )}
                Terminal
              </Button>
            )}
            {platform === "windows" && (
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                className={
                  sandbox.agent === "codex"
                    ? "border-info/40 text-info hover:bg-info/10 hover:text-info"
                    : undefined
                }
                onClick={() =>
                  run("agent", () =>
                    invoke("open_sandbox_agent", { id: sandbox.id, agent: sandbox.agent }),
                  )
                }
              >
                {busyAction === "agent" ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <TerminalSquare className="size-3.5" />
                )}
                {AGENT_LABELS[sandbox.agent as Agent] ?? sandbox.agent}
              </Button>
            )}
            {sandbox.host_port != null && (
              <Button size="sm" variant="outline" onClick={openInBrowser}>
                <ExternalLink className="size-3.5" />
                Open in browser
              </Button>
            )}
            {sandbox.mode === "clone" && (
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={gitSync}
              >
                {busyAction === "git-sync" ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <GitBranch className="size-3.5" />
                )}
                {busyAction === "git-sync" ? "Syncing…" : "Git Sync"}
              </Button>
            )}
            <Popover open={backupMenuOpen} onOpenChange={setBackupMenuOpen}>
              <PopoverTrigger asChild>
                <Button size="sm" variant="outline" disabled={busyAction != null || isBackingUp}>
                  {busyAction === "backup" ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <DatabaseBackup className="size-3.5" />
                  )}
                  {busyAction === "backup" ? "Backing up…" : "Backup"}
                </Button>
              </PopoverTrigger>
              <PopoverContent align="start" className="flex flex-col gap-0.5">
                {backupScopes.map(({ value, label }) => (
                  <button
                    key={value}
                    type="button"
                    onClick={() => {
                      setBackupMenuOpen(false)
                      handleBackupNow(value)
                    }}
                    className="flex w-full items-center justify-between gap-4 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground"
                  >
                    {label}
                  </button>
                ))}
              </PopoverContent>
            </Popover>
            <Button
              size="sm"
              variant="outline"
              disabled={busyAction != null || isBackingUp}
              onClick={() => setConfirmAction("stop")}
            >
              {busyAction === "stop" ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Square className="size-3.5" />
              )}
              {busyAction === "stop" ? "Stopping…" : "Stop"}
            </Button>
          </>
        )}
        {sandbox.status === "stopped" && (
          <Button
            size="sm"
            variant="outline"
            disabled={busyAction != null}
            onClick={() =>
              run("start", () => invoke("start_sandbox", { id: sandbox.id }))
            }
          >
            {busyAction === "start" ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Play className="size-3.5" />
            )}
            {busyAction === "start" ? "Starting…" : "Start"}
          </Button>
        )}
        <Button
          size="sm"
          variant="destructive"
          disabled={busyAction != null || isBackingUp}
          onClick={() => setConfirmAction("delete")}
        >
          {busyAction === "delete" ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <Trash2 className="size-3.5" />
          )}
          Delete
        </Button>
      </div>

      <Dialog open={confirmAction != null} onOpenChange={(open) => !open && setConfirmAction(null)}>
        <DialogContent title={confirmAction === "stop" ? "Stop sandbox?" : "Delete sandbox?"}>
          <Card className="w-full">
            <CardHeader>
              <CardTitle>{confirmAction === "stop" ? "Stop sandbox?" : "Delete sandbox?"}</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                Back up this sandbox's .claude and .git folders before {confirmAction === "stop" ? "stopping" : "deleting"} it?
              </p>
              <div className="flex flex-wrap justify-end gap-2">
                <Button variant="ghost" onClick={() => setConfirmAction(null)}>
                  Cancel
                </Button>
                <Button variant="outline" onClick={() => handleBackupChoice(false)}>
                  Skip & {confirmAction === "stop" ? "stop" : "delete"}
                </Button>
                <Button onClick={() => handleBackupChoice(true)}>
                  Back up & {confirmAction === "stop" ? "stop" : "delete"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>

      <Dialog open={notFoundDialogOpen} onOpenChange={(open) => !open && setNotFoundDialogOpen(false)}>
        <DialogContent title="Sandbox not found">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Sandbox not found</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                sbx no longer knows about this sandbox — it may have already been removed outside the
                app. Remove it from this list anyway?
              </p>
              <div className="flex flex-wrap justify-end gap-2">
                <Button variant="ghost" onClick={() => setNotFoundDialogOpen(false)}>
                  Cancel
                </Button>
                <Button variant="destructive" disabled={busyAction != null} onClick={retryDeleteAsForce}>
                  {busyAction === "delete" && <Loader2 className="size-3.5 animate-spin" />}
                  Retry
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>
    </div>
  )
}
