import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge } from '@/components/ui/badge'
import type { DetailTab, Route } from '@/lib/router'
import { formatNetworkPolicyLabel, permissionBadgeVariant } from '@/lib/sandboxDisplay'
import { useAppStore } from '@/store/useAppStore'
import { SandboxBackupsTab } from './SandboxBackupsTab'
import { SandboxBranchTab } from './SandboxBranchTab'
import { SandboxBreadcrumb } from './SandboxBreadcrumb'
import { SandboxCredentialsTab } from './SandboxCredentialsTab'
import { SandboxHeader } from './SandboxHeader'
import { SandboxMetricsTab } from './SandboxMetricsTab'
import { SandboxPlansTab } from './SandboxPlansTab'

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

  function openLocation() {
    if (location) invoke('open_path_in_explorer', { path: location })
  }

  return (
    <div className="flex flex-col gap-6">
      <SandboxHeader sandbox={sandbox} project={project} navigate={navigate} activeTab={tab} onTabSelect={setTab} />

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

      {tab === 'credentials' && <SandboxCredentialsTab sandbox={sandbox} navigate={navigate} />}
    </div>
  )
}
