import { DatabaseBackup, GitBranch } from 'lucide-react'
import type { Project } from '@/components/projects/types'
import type { DetailTab, Route } from '@/lib/router'
import { cn } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { SandboxActions } from './SandboxActions'
import { SandboxBreadcrumb, type BreadcrumbSegment } from './SandboxBreadcrumb'
import { StatusIndicator } from './StatusIndicator'
import type { Sandbox } from './types'

const TAB_LINK_CLASS =
  "-mb-px flex items-center gap-1.5 border-b-2 border-transparent px-0.5 pb-2 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
const TAB_LINK_ACTIVE_CLASS = "border-foreground text-foreground"

export function SandboxHeader({
  sandbox,
  project,
  navigate,
  activeTab,
  onTabSelect,
  breadcrumbExtra = [],
}: {
  sandbox: Sandbox
  project: Project | undefined
  navigate: (route: Route) => void
  activeTab: DetailTab | 'branches'
  onTabSelect?: (tab: DetailTab) => void
  breadcrumbExtra?: BreadcrumbSegment[]
}) {
  const isBackingUp = useAppStore((s) => s.activeBackups.some((b) => b.sandbox_id === sandbox.id))
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const title = sandbox.name ?? project?.name ?? 'Sandbox'

  function selectTab(t: DetailTab) {
    if (onTabSelect) onTabSelect(t)
    else navigate({ screen: 'sandboxes', sandboxId: sandbox.id, detailTab: t })
  }

  return (
    <div className="sticky top-0 z-10 -mt-6 flex flex-col gap-6 bg-background pt-6">
      <SandboxBreadcrumb
        segments={[
          { label: 'Sandboxes', onClick: () => navigate({ screen: 'sandboxes' }) },
          { label: title, onClick: () => navigate({ screen: 'sandboxes', sandboxId: sandbox.id }) },
          ...breadcrumbExtra,
        ]}
      />

      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <h1 className="text-lg font-medium">{title}</h1>
          <StatusIndicator status={sandbox.status} />
          {isBackingUp && (
            <button
              type="button"
              onClick={() => selectTab('backups')}
              title="Backup in progress — view details"
              className="ml-auto text-muted-foreground hover:text-foreground"
            >
              <DatabaseBackup className="size-4 animate-pulse" />
            </button>
          )}
        </div>
        {sandbox.sbx_name && <p className="text-xs text-muted-foreground">{sandbox.sbx_name}</p>}
      </div>

      <SandboxActions sandbox={sandbox} projectName={project?.name ?? 'Sandbox'} onChanged={loadSandboxes} context="detail" />

      <div className="flex gap-4 border-b border-border">
        <button type="button" onClick={() => selectTab('overview')} className={cn(TAB_LINK_CLASS, activeTab === 'overview' && TAB_LINK_ACTIVE_CLASS)}>
          Overview
        </button>
        <button
          type="button"
          onClick={() => navigate({ screen: 'sandboxes', sandboxId: sandbox.id, branches: true })}
          className={cn(TAB_LINK_CLASS, activeTab === 'branches' && TAB_LINK_ACTIVE_CLASS)}
        >
          <GitBranch className="size-3.5" />
          Branches
        </button>
        <button type="button" onClick={() => selectTab('metrics')} className={cn(TAB_LINK_CLASS, activeTab === 'metrics' && TAB_LINK_ACTIVE_CLASS)}>
          Metrics
        </button>
        <button type="button" onClick={() => selectTab('plans')} className={cn(TAB_LINK_CLASS, activeTab === 'plans' && TAB_LINK_ACTIVE_CLASS)}>
          Plans
        </button>
        <button type="button" onClick={() => selectTab('backups')} className={cn(TAB_LINK_CLASS, activeTab === 'backups' && TAB_LINK_ACTIVE_CLASS)}>
          Backups
        </button>
        <button type="button" onClick={() => selectTab('credentials')} className={cn(TAB_LINK_CLASS, activeTab === 'credentials' && TAB_LINK_ACTIVE_CLASS)}>
          Credentials
        </button>
      </div>
    </div>
  )
}
