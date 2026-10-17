import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { AGENTS, AGENT_LABELS, type Agent } from '@/lib/agentHost'
import { defaultKitId } from '@/lib/kits'
import type { NetworkRuleDecision } from '@/lib/networkPolicy'
import { SANDBOX_NETWORK_PRESET_OVERRIDE_LABELS, SANDBOX_NETWORK_PRESET_OVERRIDES } from '@/lib/networkPolicy'
import { fullPermissionMode, PERMISSION_MODES } from '@/lib/permissionModes'
import { useAppStore } from '@/store/useAppStore'
import { useCreateJobs } from '@/store/useCreateJobs'
import type { SandboxMode } from './types'

const DEFAULT_PERMISSION_MODE = '__settings_default__'
const DEFAULT_NETWORK_OVERRIDE = '__global_default__'
const NO_KIT = '__no_kit__'

export function CreateSandboxDialog({
  defaultProjectId,
  onCreated,
  onCancel,
}: {
  defaultProjectId?: string
  onCreated: () => void
  onCancel: () => void
}) {
  const projects = useAppStore((s) => s.projects)
  const sandboxes = useAppStore((s) => s.sandboxes)
  const defaultAgent = useAppStore((s) => s.defaultAgent)
  const kits = useAppStore((s) => s.kits)
  const [projectId, setProjectId] = useState(defaultProjectId ?? projects[0]?.id ?? '')
  const [kitId, setKitId] = useState<string | null>(() =>
    defaultKitId(kits, projects.find((p) => p.id === (defaultProjectId ?? projects[0]?.id)))
  )
  const projectDefaultKitId = defaultKitId(kits, projects.find((p) => p.id === projectId))
  const [name, setName] = useState('')
  const [agent, setAgent] = useState(defaultAgent)
  const [permissionMode, setPermissionMode] = useState('')
  const [mode, setMode] = useState<SandboxMode>('mount')
  const [networkPresetOverride, setNetworkPresetOverride] = useState('')
  const [pendingNetworkRules, setPendingNetworkRules] = useState<{ decision: NetworkRuleDecision; host: string }[]>([])
  const [pendingRuleDecision, setPendingRuleDecision] = useState<NetworkRuleDecision>('allow')
  const [pendingRuleHost, setPendingRuleHost] = useState('')

  const startCreate = useCreateJobs((s) => s.startCreate)
  const hasActiveMount =
    mode === 'mount' &&
    sandboxes.some(
      (sb) => sb.project_id === projectId && sb.mode === 'mount' && (sb.status === 'starting' || sb.status === 'running')
    )
  const hasStartingSandbox = sandboxes.some((sb) => sb.project_id === projectId && sb.status === 'starting')

  function addPendingNetworkRule() {
    const host = pendingRuleHost.trim()
    if (!host) return
    setPendingNetworkRules((rules) => [...rules, { decision: pendingRuleDecision, host }])
    setPendingRuleHost('')
  }

  function removePendingNetworkRule(index: number) {
    setPendingNetworkRules((rules) => rules.filter((_, i) => i !== index))
  }

  // Runs in the background; progress shows in CreateProgressPopout.
  function handleCreate() {
    const project = projects.find((p) => p.id === projectId)
    startCreate(
      {
        projectId,
        mode,
        name: name.trim() || null,
        permissionMode: permissionMode || null,
        agent,
        kitId,
        networkPresetOverride: networkPresetOverride || null,
        networkRules: pendingNetworkRules,
      },
      name.trim() || project?.name || 'sandbox',
    )
    onCreated()
  }

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>New sandbox</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-project">Project</Label>
          <Select
            value={projectId}
            onValueChange={(value) => {
              setProjectId(value)
              setKitId(defaultKitId(kits, projects.find((p) => p.id === value)))
            }}
          >
            <SelectTrigger id="sandbox-project">
              <SelectValue placeholder="Select a project" />
            </SelectTrigger>
            <SelectContent>
              {projects.map((project) => (
                <SelectItem key={project.id} value={project.id}>
                  {project.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-name">Name (optional)</Label>
          <Input
            id="sandbox-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={projects.find((p) => p.id === projectId)?.name}
          />
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-agent">Agent</Label>
          <Select
            value={agent}
            onValueChange={(value) => {
              setAgent(value)
              // Keeping the sandbox on the global default agent: leave the
              // permission mode on "use Settings default", freely changeable.
              // Overriding to a different agent: land on its full-permission
              // mode explicitly, rather than a moderate mode that could
              // surprise the user for a one-off, deliberately-chosen agent.
              setPermissionMode(value === defaultAgent ? '' : fullPermissionMode(value as Agent))
            }}
          >
            <SelectTrigger id="sandbox-agent">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {AGENTS.map((a) => (
                <SelectItem key={a} value={a}>
                  {AGENT_LABELS[a]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-kit">Kit</Label>
          <Select value={kitId ?? NO_KIT} onValueChange={(value) => setKitId(value === NO_KIT ? null : value)}>
            <SelectTrigger id="sandbox-kit">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_KIT}>No kit</SelectItem>
              {kits.map((k) => (
                <SelectItem key={k.id} value={k.id}>
                  {k.name}
                  {k.id === projectDefaultKitId ? ' (default)' : ''}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <p className="text-xs text-muted-foreground">Kit install commands run once while the sandbox is created.</p>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-permission-mode">Permission mode</Label>
          <Select
            value={permissionMode || DEFAULT_PERMISSION_MODE}
            onValueChange={(value) =>
              setPermissionMode(value === DEFAULT_PERMISSION_MODE ? '' : value)
            }
          >
            <SelectTrigger id="sandbox-permission-mode">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={DEFAULT_PERMISSION_MODE}>Use Settings default</SelectItem>
              {PERMISSION_MODES[agent as keyof typeof PERMISSION_MODES].map((m) => (
                <SelectItem key={m} value={m}>
                  {m}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <Label>Mode</Label>
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              variant={mode === 'mount' ? 'default' : 'outline'}
              onClick={() => setMode('mount')}
            >
              Mount existing folder
            </Button>
            <Button
              type="button"
              size="sm"
              variant={mode === 'clone' ? 'default' : 'outline'}
              onClick={() => setMode('clone')}
            >
              Clone fresh copy
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            {mode === 'mount'
              ? 'Runs directly against the project\'s existing local folder — edits appear on your host immediately. Only one mount-mode sandbox can run per project at a time.'
              : 'Clones the project\'s repo into an isolated copy inside the sandbox itself. Your local folder is untouched. Multiple clone-mode sandboxes can run per project.'}
          </p>
        </div>
        <div className="flex flex-col gap-1">
          <Label htmlFor="sandbox-network-preset">Network policy override</Label>
          <Select
            value={networkPresetOverride || DEFAULT_NETWORK_OVERRIDE}
            onValueChange={(value) => setNetworkPresetOverride(value === DEFAULT_NETWORK_OVERRIDE ? '' : value)}
          >
            <SelectTrigger id="sandbox-network-preset">
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
            Only Open or Locked Down can be scoped to a single sandbox — Balanced is a machine-wide baseline rule
            set with no way to apply it to just one sandbox.
          </p>
        </div>
        <div className="flex flex-col gap-1">
          <Label>Custom network rules (optional)</Label>
          {pendingNetworkRules.length > 0 && (
            <ul className="flex flex-wrap gap-1.5">
              {pendingNetworkRules.map((rule, index) => (
                <li
                  key={index}
                  className="flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-xs"
                >
                  <span>{rule.decision}</span>
                  <span className="font-mono">{rule.host}</span>
                  <button
                    type="button"
                    onClick={() => removePendingNetworkRule(index)}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    ×
                  </button>
                </li>
              ))}
            </ul>
          )}
          <div className="flex gap-2">
            <Select value={pendingRuleDecision} onValueChange={(v) => setPendingRuleDecision(v as NetworkRuleDecision)}>
              <SelectTrigger className="w-24">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="allow">Allow</SelectItem>
                <SelectItem value="deny">Deny</SelectItem>
              </SelectContent>
            </Select>
            <Input
              placeholder="host, e.g. api.example.com"
              value={pendingRuleHost}
              onChange={(e) => setPendingRuleHost(e.target.value)}
            />
            <Button type="button" size="sm" variant="outline" onClick={addPendingNetworkRule}>
              Add
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">Applied to this sandbox once it's created.</p>
        </div>
        {hasActiveMount && (
          <p className="text-xs text-destructive">A mount-mode sandbox is already running for this project.</p>
        )}
        {!hasActiveMount && hasStartingSandbox && (
          <p className="text-xs text-destructive">A sandbox is already starting for this project.</p>
        )}
        <div className="flex gap-2">
          <Button
            className="flex-1"
            onClick={handleCreate}
            disabled={!projectId || hasActiveMount || hasStartingSandbox}
          >
            {hasActiveMount ? 'Sandbox running' : hasStartingSandbox ? 'Sandbox starting…' : 'Start sandbox'}
          </Button>
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
