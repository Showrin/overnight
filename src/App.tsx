import { useCallback, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Titlebar } from '@/components/titlebar'
import { Sidebar } from '@/components/sidebar'
import { OperationsIndicator } from '@/components/OperationsIndicator'
import { BackupsScreen } from '@/components/backups/BackupsScreen'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'
import { ProjectsScreen } from '@/components/projects/ProjectsScreen'
import { HostStatsBar } from '@/components/sandboxes/HostStatsBar'
import { CreateProgressPopout } from '@/components/sandboxes/CreateProgressPopout'
import { GitSyncToast } from '@/components/sandboxes/GitSyncToast'
import { SandboxesScreen } from '@/components/sandboxes/SandboxesScreen'
import { SettingsDialog } from '@/components/settings/SettingsDialog'
import { parseRoute, pushRoute, type Route } from '@/lib/router'
import { initAppStore, useAppStore } from '@/store/useAppStore'
import { initCreateJobs } from '@/store/useCreateJobs'
import { cn } from '@/lib/utils'
import type { BranchSyncOutcome } from '@/components/sandboxes/types'

function App() {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash))
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const layoutExpanded = useAppStore((state) => state.layoutExpanded)
  const saveLayoutExpanded = useAppStore((state) => state.saveLayoutExpanded)

  const navigate = useCallback((next: Route) => {
    pushRoute(next)
    setRoute(next)
  }, [])

  useEffect(() => {
    initAppStore()
    initCreateJobs()

    const splash = document.getElementById('splash')
    if (splash) {
      const MIN_SPLASH_MS = 2500
      const remaining = Math.max(0, MIN_SPLASH_MS - performance.now())
      const timer = setTimeout(() => {
        splash.classList.add('splash-hidden')
        splash.addEventListener('transitionend', () => splash.remove(), { once: true })
      }, remaining)
      return () => clearTimeout(timer)
    }
  }, [])

  useEffect(() => {
    function onPopState() {
      setRoute(parseRoute(window.location.hash))
    }
    window.addEventListener('popstate', onPopState)
    return () => window.removeEventListener('popstate', onPopState)
  }, [])

  useEffect(() => {
    // Auto sync only interrupts when a branch needs a manual merge.
    const unlisten = listen<{ sandbox_id: string; outcomes: BranchSyncOutcome[] }>('git-sync-auto', (event) => {
      const { sandbox_id, outcomes } = event.payload
      useAppStore.getState().loadSandboxes()
      if (!outcomes.some((o) => o.status === 'needs_manual_merge')) return
      const sandbox = useAppStore.getState().sandboxes.find((s) => s.id === sandbox_id)
      useAppStore.getState().showGitSyncToast({
        sandboxId: sandbox_id,
        label: `Auto Git Sync — ${sandbox?.name ?? sandbox?.sbx_name ?? 'sandbox'}`,
        outcomes,
      })
    })
    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  useEffect(() => {
    const unlisten = listen<{ sandboxId: string; branches: boolean }>('notification-clicked', (event) => {
      navigate({ screen: 'sandboxes', sandboxId: event.payload.sandboxId, branches: event.payload.branches })
    })
    return () => {
      unlisten.then((fn) => fn())
    }
  }, [navigate])

  // The sandbox list page has its own stats panel.
  const showStatsBar = !(route.screen === 'sandboxes' && !route.sandboxId)

  return (
    <div className="flex h-screen flex-col bg-background">
      <Titlebar
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed((collapsed) => !collapsed)}
        layoutExpanded={layoutExpanded}
        onToggleLayoutExpanded={() => saveLayoutExpanded(!layoutExpanded)}
      />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar current={route.screen} onNavigate={navigate} collapsed={sidebarCollapsed} />
        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
        <main className="flex flex-1 justify-center overflow-auto">
          {route.screen === 'sandboxes' && route.branch ? (
            <div className="@container flex h-full w-full flex-col overflow-hidden p-6">
              <SandboxesScreen
                sandboxId={route.sandboxId}
                branches={route.branches}
                branch={route.branch}
                navigate={navigate}
              />
            </div>
          ) : route.screen === 'sandboxes' && route.sandboxId && !route.branches ? (
            <div className={cn('@container flex h-full w-full flex-col p-6', !layoutExpanded && 'max-w-5xl')}>
              <SandboxesScreen
                sandboxId={route.sandboxId}
                branches={route.branches}
                detailTab={route.detailTab}
                navigate={navigate}
              />
            </div>
          ) : (
            <div className={cn('@container w-full h-fit p-6', !layoutExpanded && 'max-w-5xl')}>
              {route.screen === 'dashboard' && <JiraIssueList />}
              {route.screen === 'projects' && <ProjectsScreen />}
              {route.screen === 'sandboxes' && (
                <SandboxesScreen
                  sandboxId={route.sandboxId}
                  branches={route.branches}
                  detailTab={route.detailTab}
                  navigate={navigate}
                />
              )}
              {route.screen === 'backups' && <BackupsScreen />}
            </div>
          )}
        </main>
        {showStatsBar && <HostStatsBar />}
        </div>
      </div>
      <OperationsIndicator navigate={navigate} />
      <CreateProgressPopout />
      <GitSyncToast />
      <SettingsDialog />
    </div>
  )
}

export default App
