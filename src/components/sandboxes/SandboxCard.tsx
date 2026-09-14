import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, ChevronDown, Code, DatabaseBackup, ExternalLink, GitBranch, Loader2, Play, Square, TerminalSquare, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { NetworkRuleEditor } from '@/components/settings/NetworkRuleEditor'
import type { Route } from '@/lib/router'
import type { NetworkRuleDecision, PolicyRule, SandboxNetworkPresetOverride } from '@/lib/networkPolicy'
import {
  NETWORK_POLICY_PRESET_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDES,
} from '@/lib/networkPolicy'
import { notify } from '@/lib/notify'
import { formatRelativeTime, permissionBadgeVariant, statusBadgeVariant } from '@/lib/sandboxDisplay'
import { TERMINAL_HOSTS, TERMINAL_HOST_LABELS, TERMINAL_HOST_OPEN_LABELS, type TerminalHost } from '@/lib/terminalHost'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import type { BranchSyncOutcome, Sandbox } from './types'

const DEFAULT_NETWORK_OVERRIDE = '__global_default__'

const BACKUP_SCOPES: { value: 'all' | 'claude' | 'git'; label: string }[] = [
  { value: 'all', label: 'Backup all' },
  { value: 'claude', label: 'Backup .claude' },
  { value: 'git', label: 'Backup .git' },
]

type BusyAction = 'start' | 'stop' | 'delete' | 'vscode' | 'terminal' | 'git-sync' | 'backup' | null
type ConfirmAction = 'stop' | 'delete' | null

function branchSyncStatusLabel(status: BranchSyncOutcome['status']): string {
  switch (status) {
    case 'fast_forwarded':
      return 'fast-forwarded'
    case 'needs_manual_merge':
      return 'diverged — needs manual merge'
    case 'new_branch':
      return 'new branch available (fetched, not checked out)'
  }
}

export function SandboxCard({
  sandbox,
  projectName,
  onChanged,
  onSelect,
  navigate,
}: {
  sandbox: Sandbox
  projectName: string
  onChanged: () => void
  onSelect: () => void
  navigate: (route: Route) => void
}) {
  const projectRepoPath = useAppStore(
    (s) => s.projects.find((p) => p.id === sandbox.project_id)?.repo_path
  )
  const defaultTerminalHost = useAppStore((s) => s.defaultTerminalHost)
  const saveDefaultTerminalHost = useAppStore((s) => s.saveDefaultTerminalHost)
  const platform = useAppStore((s) => s.platform)
  const networkPolicyPreset = useAppStore((s) => s.networkPolicyPreset)
  const isBackingUp = useAppStore((s) => s.activeBackups.some((b) => b.sandbox_id === sandbox.id))
  const [busyAction, setBusyAction] = useState<BusyAction>(null)
  const [error, setError] = useState<string | null>(null)
  const [copiedInfo, setCopiedInfo] = useState(false)
  const [gitSyncResult, setGitSyncResult] = useState<BranchSyncOutcome[] | null>(null)
  const [terminalMenuOpen, setTerminalMenuOpen] = useState(false)
  const [backupMenuOpen, setBackupMenuOpen] = useState(false)
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>(null)
  const [notFoundDialogOpen, setNotFoundDialogOpen] = useState(false)

  const [networkRules, setNetworkRules] = useState<PolicyRule[]>([])
  const [loadingNetworkRules, setLoadingNetworkRules] = useState(false)
  const [networkExpanded, setNetworkExpanded] = useState(false)
  const [savingNetworkOverride, setSavingNetworkOverride] = useState(false)
  const [networkError, setNetworkError] = useState<string | null>(null)

  async function refreshNetworkRules() {
    setLoadingNetworkRules(true)
    setNetworkError(null)
    try {
      const rules = await invoke<PolicyRule[]>('get_sandbox_network_rules', { id: sandbox.id })
      setNetworkRules(rules)
    } catch (e) {
      setNetworkError(String(e))
    } finally {
      setLoadingNetworkRules(false)
    }
  }

  // Loaded once on mount (not on the shared 5s sandbox poll) — every
  // visible card's rule count means one `sbx policy ls --wide` call, and
  // rules only ever change from actions this same card triggers, so a
  // reload after those is enough to stay fresh.
  useEffect(() => {
    refreshNetworkRules()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sandbox.id])

  async function addSandboxNetworkRule(decision: NetworkRuleDecision, host: string) {
    await invoke('add_sandbox_network_rule', { id: sandbox.id, decision, host })
    await refreshNetworkRules()
  }

  async function removeSandboxNetworkRule(host: string) {
    await invoke('remove_sandbox_network_rule', { id: sandbox.id, host })
    await refreshNetworkRules()
  }

  async function handleNetworkOverrideChange(value: string) {
    const preset = value === DEFAULT_NETWORK_OVERRIDE ? null : value
    setSavingNetworkOverride(true)
    setNetworkError(null)
    try {
      await invoke('set_sandbox_network_preset_override', { id: sandbox.id, preset })
      await refreshNetworkRules()
      onChanged()
    } catch (e) {
      setNetworkError(String(e))
    } finally {
      setSavingNetworkOverride(false)
    }
  }

  const location = sandbox.folder_path ?? projectRepoPath

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

  async function copyInfoCommand() {
    if (!sandbox.sbx_name) return
    await navigator.clipboard.writeText(sandbox.sbx_name)
    setCopiedInfo(true)
    setTimeout(() => setCopiedInfo(false), 1500)
  }

  function openLocation() {
    if (location) invoke('open_path_in_explorer', { path: location })
  }

  async function handleBackupNow(scope: 'all' | 'claude' | 'git') {
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
      setGitSyncResult(outcomes)
      const fastForwarded = outcomes.filter((o) => o.status === 'fast_forwarded').length
      notify('Git Sync complete', `${fastForwarded} of ${outcomes.length} branch(es) fast-forwarded.`)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusyAction(null)
    }
  }

  return (
    <Card
      className={cn('gap-6', isBackingUp && 'ring-2 ring-primary/50 shadow-md shadow-primary/30 transition-shadow')}
      size="sm"
    >
      <CardHeader className="gap-1.5">
        <div className="flex items-center gap-2">
          <CardTitle>
            <button
              type="button"
              onClick={onSelect}
              className="text-left hover:underline"
              title="View sandbox details"
            >
              {sandbox.name ?? projectName}
            </button>
          </CardTitle>
          <Badge variant={statusBadgeVariant(sandbox.status)}>
            {sandbox.status}
          </Badge>
          {isBackingUp && (
            <button
              type="button"
              onClick={() => navigate({ screen: 'sandboxes', sandboxId: sandbox.id, detailTab: 'backups' })}
              title="Backup in progress — view details"
              className="ml-auto text-muted-foreground hover:text-foreground"
            >
              <DatabaseBackup className="size-4 animate-pulse" />
            </button>
          )}
        </div>
        {sandbox.sbx_name && (
          <button
            type="button"
            onClick={copyInfoCommand}
            title="Copy `sbx info` command"
            className="w-fit text-left text-xs text-muted-foreground hover:text-foreground hover:underline"
          >
            {copiedInfo ? "Copied!" : sandbox.sbx_name}
          </button>
        )}
      </CardHeader>
      <CardContent className="flex flex-col gap-6 text-sm text-muted-foreground">
        <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-xs text-muted-foreground/70">
              Permission Mode
            </span>
            <Badge
              variant={permissionBadgeVariant(sandbox.permission_mode)}
              className="w-fit"
            >
              {sandbox.permission_mode}
            </Badge>
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-xs text-muted-foreground/70">
              Project Attachment
            </span>
            <span className="text-sm text-foreground">
              {sandbox.mode === "clone" ? "Cloned" : "Mounted"}
            </span>
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-xs text-muted-foreground/70">Location</span>
            {location ? (
              <button
                type="button"
                onClick={openLocation}
                title={`Open ${location} in the file explorer`}
                className="truncate text-left text-sm text-foreground hover:underline"
              >
                {location}
              </button>
            ) : (
              <span className="text-sm">—</span>
            )}
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-xs text-muted-foreground/70">Network</span>
            <div className="flex items-center gap-1.5">
              <span className="truncate text-sm text-foreground">
                {sandbox.network_preset_override
                  ? SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS[
                      sandbox.network_preset_override as SandboxNetworkPresetOverride
                    ] ?? sandbox.network_preset_override
                  : networkPolicyPreset
                    ? `Global default (${NETWORK_POLICY_PRESET_LABELS[networkPolicyPreset as keyof typeof NETWORK_POLICY_PRESET_LABELS] ?? networkPolicyPreset})`
                    : 'Global default'}
              </span>
              <button
                type="button"
                onClick={() => setNetworkExpanded((v) => !v)}
                className="shrink-0 text-xs text-muted-foreground hover:text-foreground hover:underline"
              >
                {loadingNetworkRules ? '…' : `${networkRules.length} rule(s)`}
              </button>
            </div>
          </div>
        </div>

        {networkExpanded && (
          <div className="flex flex-col gap-3 rounded-lg border border-border p-3">
            <div className="flex flex-col gap-1">
              <span className="text-xs text-muted-foreground/70">Preset override</span>
              <Select
                value={sandbox.network_preset_override ?? DEFAULT_NETWORK_OVERRIDE}
                onValueChange={handleNetworkOverrideChange}
                disabled={savingNetworkOverride}
              >
                <SelectTrigger className="h-8 w-48">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={DEFAULT_NETWORK_OVERRIDE}>Use global default</SelectItem>
                  {SANDBOX_NETWORK_PRESET_OVERRIDES.map((preset) => (
                    <SelectItem key={preset} value={preset}>
                      {SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS[preset]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground">
                Only Open or Locked Down can be scoped to a single sandbox — Balanced is a machine-wide baseline
                rule set.
              </p>
            </div>
            <NetworkRuleEditor rules={networkRules} onAdd={addSandboxNetworkRule} onRemove={removeSandboxNetworkRule} />
            {networkError && <p className="text-xs text-destructive">{networkError}</p>}
          </div>
        )}

        {sandbox.last_backup_at != null && (
          <p className="text-xs">Last backup: {formatRelativeTime(sandbox.last_backup_at)}</p>
        )}

        {error && <p className="text-destructive">{error}</p>}

        {gitSyncResult && (
          <div className="rounded-md border p-3 text-xs">
            <p className="font-medium text-foreground">Git Sync result</p>
            {gitSyncResult.length === 0 ? (
              <p className="mt-1">No sandbox branches to sync.</p>
            ) : (
              <ul className="mt-1 flex flex-col gap-0.5">
                {gitSyncResult.map((o) => (
                  <li key={o.branch}>
                    <span className="font-mono text-foreground">{o.branch}</span>
                    {" — "}
                    {branchSyncStatusLabel(o.status)}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

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
                  {BACKUP_SCOPES.map(({ value, label }) => (
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
      </CardContent>

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
    </Card>
  );
}
