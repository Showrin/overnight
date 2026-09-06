import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { ArrowLeft } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { formatNetworkPolicyLabel, permissionBadgeVariant, statusBadgeVariant } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import { SandboxBranchTab } from './SandboxBranchTab'
import { SandboxDiffTab } from './SandboxDiffTab'
import { SandboxMetricsTab } from './SandboxMetricsTab'

type DetailTab = 'overview' | 'branch' | 'metrics' | 'diff'

const TABS: { id: DetailTab; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'branch', label: 'Branch' },
  { id: 'metrics', label: 'Metrics' },
  { id: 'diff', label: 'Diff' },
]

export function SandboxDetailScreen({
  sandboxId,
  onBack,
}: {
  sandboxId: string
  onBack: () => void
}) {
  const [tab, setTab] = useState<DetailTab>('overview')
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === sandboxId))
  const project = useAppStore((s) => s.projects.find((p) => p.id === sandbox?.project_id))
  const networkPolicyPreset = useAppStore((s) => s.networkPolicyPreset)

  const backLink = (
    <Button variant="link" size="sm" className="h-auto w-fit gap-1 px-0" onClick={onBack}>
      <ArrowLeft className="size-3.5" />
      Sandboxes
    </Button>
  )

  if (!sandbox) {
    return (
      <div className="flex flex-col gap-4">
        {backLink}
        <p className="text-sm text-muted-foreground">
          This sandbox couldn't be found — it may have been deleted.
        </p>
      </div>
    )
  }

  const location = sandbox.folder_path ?? project?.repo_path
  const title = sandbox.name ?? project?.name ?? 'Sandbox'

  function openLocation() {
    if (location) invoke('open_path_in_explorer', { path: location })
  }

  return (
    <div className="flex flex-col gap-6">
      {backLink}

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-medium">{title}</h1>
          <Badge variant={statusBadgeVariant(sandbox.status)}>{sandbox.status}</Badge>
        </div>
        {sandbox.sbx_name && <p className="text-xs text-muted-foreground">{sandbox.sbx_name}</p>}
      </div>

      <div className="flex gap-2 border-b border-border pb-2">
        {TABS.map((t) => (
          <Button
            key={t.id}
            type="button"
            size="sm"
            variant={tab === t.id ? 'default' : 'outline'}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </Button>
        ))}
      </div>

      {tab === 'overview' && (
        <div className="grid grid-cols-2 gap-4 text-sm text-muted-foreground md:grid-cols-4">
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-xs text-muted-foreground/70">Permission Mode</span>
            <Badge variant={permissionBadgeVariant(sandbox.permission_mode)} className="w-fit">
              {sandbox.permission_mode}
            </Badge>
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-xs text-muted-foreground/70">Project Attachment</span>
            <span className="text-sm text-foreground">{sandbox.mode === 'clone' ? 'Cloned' : 'Mounted'}</span>
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
            <span className="truncate text-sm text-foreground">
              {formatNetworkPolicyLabel(sandbox.network_preset_override, networkPolicyPreset)}
            </span>
          </div>
        </div>
      )}

      {tab === 'branch' && <SandboxBranchTab sandbox={sandbox} />}

      {tab === 'metrics' && <SandboxMetricsTab sandbox={sandbox} />}

      {tab === 'diff' && <SandboxDiffTab sandbox={sandbox} />}
    </div>
  )
}
