import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { DatabaseBackup, GitBranch } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import type { DetailTab, Route } from '@/lib/router'
import { formatNetworkPolicyLabel, permissionBadgeVariant, statusBadgeVariant } from '@/lib/sandboxDisplay'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { SandboxActions } from './SandboxActions'
import { SandboxBackupsTab } from './SandboxBackupsTab'
import { SandboxBranchTab } from './SandboxBranchTab'
import { SandboxBreadcrumb } from './SandboxBreadcrumb'
import { SandboxMetricsTab } from './SandboxMetricsTab'
import { SandboxPlansTab } from './SandboxPlansTab'

const TAB_LINK_CLASS =
  "-mb-px flex items-center gap-1.5 border-b-2 border-transparent px-0.5 pb-2 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
const TAB_LINK_ACTIVE_CLASS = "border-foreground text-foreground"

export function SandboxDetailScreen({
  sandboxId,
  navigate,
  initialDetailTab,
}: {
  sandboxId: string
  navigate: (route: Route) => void
  initialDetailTab?: DetailTab
}) {
  const [tab, setTab] = useState<DetailTab>(initialDetailTab ?? 'overview')
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === sandboxId))
  const project = useAppStore((s) => s.projects.find((p) => p.id === sandbox?.project_id))
  const networkPolicyPreset = useAppStore((s) => s.networkPolicyPreset)
  const isBackingUp = useAppStore((s) => s.activeBackups.some((b) => b.sandbox_id === sandboxId))
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)

  const toSandboxes = () => navigate({ screen: 'sandboxes' })

  if (!sandbox) {
    return (
      <div className="flex flex-col gap-4">
        <SandboxBreadcrumb segments={[{ label: 'Sandboxes', onClick: toSandboxes }]} />
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
      <SandboxBreadcrumb segments={[{ label: 'Sandboxes', onClick: toSandboxes }, { label: title }]} />

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-medium">{title}</h1>
          <Badge variant={statusBadgeVariant(sandbox.status)}>{sandbox.status}</Badge>
          {isBackingUp && (
            <button
              type="button"
              onClick={() => setTab('backups')}
              title="Backup in progress — view details"
              className="ml-auto text-muted-foreground hover:text-foreground"
            >
              <DatabaseBackup className="size-4 animate-pulse" />
            </button>
          )}
        </div>
        {sandbox.sbx_name && <p className="text-xs text-muted-foreground">{sandbox.sbx_name}</p>}
      </div>

      <SandboxActions sandbox={sandbox} projectName={project?.name ?? 'Sandbox'} onChanged={loadSandboxes} />

      <div className="flex gap-4 border-b border-border">
        <button
          type="button"
          onClick={() => setTab('overview')}
          className={cn(TAB_LINK_CLASS, tab === 'overview' && TAB_LINK_ACTIVE_CLASS)}
        >
          Overview
        </button>
        <button
          type="button"
          onClick={() => navigate({ screen: 'sandboxes', sandboxId: sandbox.id, branches: true })}
          className={TAB_LINK_CLASS}
        >
          <GitBranch className="size-3.5" />
          Branches
        </button>
        <button
          type="button"
          onClick={() => setTab('metrics')}
          className={cn(TAB_LINK_CLASS, tab === 'metrics' && TAB_LINK_ACTIVE_CLASS)}
        >
          Metrics
        </button>
        <button
          type="button"
          onClick={() => setTab('plans')}
          className={cn(TAB_LINK_CLASS, tab === 'plans' && TAB_LINK_ACTIVE_CLASS)}
        >
          Plans
        </button>
        <button
          type="button"
          onClick={() => setTab('backups')}
          className={cn(TAB_LINK_CLASS, tab === 'backups' && TAB_LINK_ACTIVE_CLASS)}
        >
          Backups
        </button>
      </div>

      {tab === 'overview' && (
        <div className="flex flex-col gap-6">
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

          <SandboxBranchTab sandbox={sandbox} />
        </div>
      )}

      {tab === 'metrics' && <SandboxMetricsTab sandbox={sandbox} />}

      {tab === 'plans' && <SandboxPlansTab sandbox={sandbox} />}

      {tab === 'backups' && <SandboxBackupsTab sandbox={sandbox} />}
    </div>
  )
}
