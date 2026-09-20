import { useCallback, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Titlebar } from '@/components/titlebar'
import { Sidebar } from '@/components/sidebar'
import { OperationsIndicator } from '@/components/OperationsIndicator'
import { BackupsScreen } from '@/components/backups/BackupsScreen'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'
import { ProjectsScreen } from '@/components/projects/ProjectsScreen'
import { PerformanceMonitorTab } from '@/components/sandboxes/PerformanceMonitorTab'
import { SandboxesScreen } from '@/components/sandboxes/SandboxesScreen'
import { SettingsScreen } from '@/components/settings/SettingsScreen'
import { TelemetryTab } from '@/components/developer/TelemetryTab'
import { CommandLogTab } from '@/components/developer/CommandLogTab'
import { parseRoute, pushRoute, type Route } from '@/lib/router'
import { initAppStore, useAppStore } from '@/store/useAppStore'
import { cn } from '@/lib/utils'

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
    const unlisten = listen<string>('notification-clicked', (event) => {
      navigate({ screen: 'sandboxes', sandboxId: event.payload })
    })
    return () => {
      unlisten.then((fn) => fn())
    }
  }, [navigate])

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
          ) : route.screen === 'dev-telemetry' || route.screen === 'dev-commands' || route.screen === 'settings' ? (
            <div className={cn('@container flex h-full w-full flex-col p-6', !layoutExpanded && 'max-w-5xl')}>
              {route.screen === 'dev-telemetry' && <TelemetryTab />}
              {route.screen === 'dev-commands' && <CommandLogTab />}
              {route.screen === 'settings' && (
                <SettingsScreen settingsTab={route.settingsTab ?? 'general'} navigate={navigate} />
              )}
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
              {route.screen === 'performance-monitor' && <PerformanceMonitorTab />}
              {route.screen === 'backups' && <BackupsScreen />}
            </div>
          )}
        </main>
      </div>
      <OperationsIndicator navigate={navigate} />
    </div>
  )
}

export default App
