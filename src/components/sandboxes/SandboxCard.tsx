import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, Code, DatabaseBackup, ExternalLink, FolderOpen, GitBranch, Loader2, Play, Square, TerminalSquare, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { NetworkRuleEditor } from '@/components/settings/NetworkRuleEditor'
import type { NetworkRuleDecision, PolicyRule, SandboxNetworkPresetOverride } from '@/lib/networkPolicy'
import {
  NETWORK_POLICY_PRESET_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDES,
} from '@/lib/networkPolicy'
import { notify } from '@/lib/notify'
import { permissionBadgeVariant, statusBadgeVariant } from '@/lib/sandboxDisplay'
import { TERMINAL_HOSTS, TERMINAL_HOST_LABELS } from '@/lib/terminalHost'
import { useAppStore } from '@/store/useAppStore'
import type { BranchSyncOutcome, Sandbox } from './types'

const DEFAULT_NETWORK_OVERRIDE = '__global_default__'

type BusyAction = 'start' | 'stop' | 'delete' | 'vscode' | 'terminal' | 'git-sync' | 'backup' | null

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

// Formats a past unix-ms timestamp as "3 minutes ago"-style relative text,
// using the platform's own `Intl.RelativeTimeFormat` rather than a new
// dependency for one caption.
function formatRelativeTime(pastMs: number): string {
  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' })
  const divisions: [Intl.RelativeTimeFormatUnit, number][] = [
    ['second', 60],
    ['minute', 60],
    ['hour', 24],
    ['day', 30],
    ['month', 12],
    ['year', Infinity],
  ]
  let duration = (pastMs - Date.now()) / 1000
  for (const [unit, amount] of divisions) {
    if (Math.abs(duration) < amount) {
      return rtf.format(Math.round(duration), unit)
    }
    duration /= amount
  }
  return rtf.format(Math.round(duration), 'year')
}

export function SandboxCard({
  sandbox,
  projectName,
  onChanged,
  onSelect,
}: {
  sandbox: Sandbox
  projectName: string
  onChanged: () => void
  onSelect: () => void
}) {
  const projectRepoPath = useAppStore(
    (s) => s.projects.find((p) => p.id === sandbox.project_id)?.repo_path
  )
  const defaultTerminalHost = useAppStore((s) => s.defaultTerminalHost)
  const platform = useAppStore((s) => s.platform)
  const networkPolicyPreset = useAppStore((s) => s.networkPolicyPreset)
  const [busyAction, setBusyAction] = useState<BusyAction>(null)
  const [error, setError] = useState<string | null>(null)
  const [copiedInfo, setCopiedInfo] = useState(false)
  const [gitSyncResult, setGitSyncResult] = useState<BranchSyncOutcome[] | null>(null)
  const [terminalMenuOpen, setTerminalMenuOpen] = useState(false)

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
      setError(String(e))
    } finally {
      setBusyAction(null)
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

  function openBackupFolder() {
    if (sandbox.last_backup_path) invoke('open_path_in_explorer', { path: sandbox.last_backup_path })
  }

  async function backupClaudeData() {
    setBusyAction('backup')
    setError(null)
    try {
      await invoke('backup_sandbox_claude_data', { id: sandbox.id })
      notify('Claude data backed up', `${sandbox.name ?? projectName}'s ~/.claude has been copied to the host.`, sandbox.id)
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
    <Card className="gap-6" size="sm">
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
          <p className="text-xs">
            Last backup: {formatRelativeTime(sandbox.last_backup_at)}
            {sandbox.last_backup_path && (
              <>
                {" — "}
                <button
                  type="button"
                  onClick={openBackupFolder}
                  className="inline-flex items-center gap-1 text-left hover:text-foreground hover:underline"
                >
                  <FolderOpen className="size-3" />
                  Open folder
                </button>
              </>
            )}
          </p>
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
                Open in VS Code
              </Button>
              {platform === "windows" ? (
                <Popover open={terminalMenuOpen} onOpenChange={setTerminalMenuOpen}>
                  <PopoverTrigger asChild>
                    <Button size="sm" variant="outline" disabled={busyAction != null}>
                      {busyAction === "terminal" ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : (
                        <TerminalSquare className="size-3.5" />
                      )}
                      Terminal
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent align="start" className="flex flex-col gap-0.5">
                    {TERMINAL_HOSTS.map((host) => (
                      <button
                        key={host}
                        type="button"
                        onClick={() => {
                          setTerminalMenuOpen(false)
                          run("terminal", () =>
                            invoke("open_sandbox_terminal", { id: sandbox.id, terminalHost: host }),
                          )
                        }}
                        className="flex w-full items-center justify-between gap-4 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground"
                      >
                        {TERMINAL_HOST_LABELS[host]}
                        {host === defaultTerminalHost && <Check className="size-3.5" />}
                      </button>
                    ))}
                  </PopoverContent>
                </Popover>
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
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={backupClaudeData}
              >
                {busyAction === "backup" ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <DatabaseBackup className="size-3.5" />
                )}
                {busyAction === "backup" ? "Backing up…" : "Backup .claude"}
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={busyAction != null}
                onClick={() =>
                  run("stop", () => invoke("stop_sandbox", { id: sandbox.id }))
                }
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
            disabled={busyAction != null}
            onClick={() =>
              run("delete", () => invoke("delete_sandbox", { id: sandbox.id }))
            }
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
    </Card>
  );
}
