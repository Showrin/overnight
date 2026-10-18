import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { DatabaseBackup } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { Route } from '@/lib/router'
import { formatRelativeTime, networkModeLabel, permissionTextClass } from '@/lib/sandboxDisplay'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { SandboxActions } from './SandboxActions'
import { SetupFailuresBanner } from './SetupFailuresBanner'
import { StatusIndicator } from './StatusIndicator'
import type { Sandbox } from './types'

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
          <StatusIndicator status={sandbox.status} />
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
            <span className={cn('truncate text-sm', permissionTextClass(sandbox.agent, sandbox.permission_mode))}>
              {sandbox.permission_mode}
            </span>
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
            <span className="truncate text-sm text-foreground">
              {networkModeLabel(sandbox.network_preset_override, networkPolicyPreset)}
            </span>
          </div>
        </div>

        {sandbox.last_backup_at != null && (
          <p className="text-xs">Last backup: {formatRelativeTime(sandbox.last_backup_at)}</p>
        )}

        <SetupFailuresBanner sandbox={sandbox} />

        <SandboxActions sandbox={sandbox} projectName={projectName} onChanged={onChanged} context="card" />
      </CardContent>
    </Card>
  );
}
