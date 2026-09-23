import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { DatabaseBackup } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { NetworkRuleEditor } from '@/components/settings/NetworkRuleEditor'
import type { Route } from '@/lib/router'
import type { NetworkRuleDecision, PolicyRule, SandboxNetworkPresetOverride } from '@/lib/networkPolicy'
import {
  NETWORK_POLICY_PRESET_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS,
  SANDBOX_NETWORK_PRESET_OVERRIDES,
} from '@/lib/networkPolicy'
import { formatRelativeTime, permissionBadgeVariant, statusBadgeVariant } from '@/lib/sandboxDisplay'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { SandboxActions } from './SandboxActions'
import type { Sandbox } from './types'

const DEFAULT_NETWORK_OVERRIDE = '__global_default__'

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
  const networkPolicyPreset = useAppStore((s) => s.networkPolicyPreset)
  const isBackingUp = useAppStore((s) => s.activeBackups.some((b) => b.sandbox_id === sandbox.id))
  const [copiedInfo, setCopiedInfo] = useState(false)

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

  async function copyInfoCommand() {
    if (!sandbox.sbx_name) return
    await navigator.clipboard.writeText(sandbox.sbx_name)
    setCopiedInfo(true)
    setTimeout(() => setCopiedInfo(false), 1500)
  }

  function openLocation() {
    if (location) invoke('open_path_in_explorer', { path: location })
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
              variant={permissionBadgeVariant(sandbox.agent, sandbox.permission_mode)}
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

        <SandboxActions sandbox={sandbox} projectName={projectName} onChanged={onChanged} context="card" />
      </CardContent>
    </Card>
  );
}
