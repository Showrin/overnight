import { useCallback, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Titlebar } from '@/components/titlebar'
import { Sidebar } from '@/components/sidebar'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'
import { ProjectsScreen } from '@/components/projects/ProjectsScreen'
import { PerformanceMonitorTab } from '@/components/sandboxes/PerformanceMonitorTab'
import { SandboxesScreen } from '@/components/sandboxes/SandboxesScreen'
import { SettingsScreen } from '@/components/settings/SettingsScreen'
import { parseRoute, pushRoute, type Route } from '@/lib/router'
import { initAppStore } from '@/store/useAppStore'

function App() {
  const [route, setRoute] = useState<Route>(() => parseRoute(window.location.hash))
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)

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
      />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar current={route.screen} onNavigate={navigate} collapsed={sidebarCollapsed} />
        <main className="flex flex-1 justify-center overflow-auto">
          <div className="@container w-full max-w-5xl h-fit p-6">
            {route.screen === 'dashboard' && <JiraIssueList />}
            {route.screen === 'projects' && <ProjectsScreen />}
            {route.screen === 'sandboxes' && (
              <SandboxesScreen sandboxId={route.sandboxId} navigate={navigate} />
            )}
            {route.screen === 'performance-monitor' && <PerformanceMonitorTab />}
            {route.screen === 'settings' && <SettingsScreen />}
          </div>
        </main>
      </div>
    </div>
  )
}

export default App
