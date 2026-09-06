import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Titlebar } from '@/components/titlebar'
import { Sidebar, type Screen } from '@/components/sidebar'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'
import { ProjectsScreen } from '@/components/projects/ProjectsScreen'
import { SandboxesScreen } from '@/components/sandboxes/SandboxesScreen'
import { SettingsScreen } from '@/components/settings/SettingsScreen'
import { initAppStore } from '@/store/useAppStore'

// Matches the glow-pulse animation's duration (1.2s x 3) in index.css.
const GLOW_DURATION_MS = 3600

function App() {
  const [screen, setScreen] = useState<Screen>('projects')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [highlightedSandboxId, setHighlightedSandboxId] = useState<string | null>(null)

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
    const unlisten = listen<string>('notification-clicked', (event) => {
      setScreen('sandboxes')
      setHighlightedSandboxId(event.payload)
      setTimeout(() => setHighlightedSandboxId(null), GLOW_DURATION_MS)
    })
    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  return (
    <div className="flex h-screen flex-col bg-background">
      <Titlebar
        sidebarCollapsed={sidebarCollapsed}
        onToggleSidebar={() => setSidebarCollapsed((collapsed) => !collapsed)}
      />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar current={screen} onNavigate={setScreen} collapsed={sidebarCollapsed} />
        <main className="flex flex-1 justify-center overflow-auto">
          <div className="@container w-full max-w-[1024px] p-6">
            {screen === 'dashboard' && <JiraIssueList />}
            {screen === 'projects' && <ProjectsScreen />}
            {screen === 'sandboxes' && <SandboxesScreen highlightedSandboxId={highlightedSandboxId} />}
            {screen === 'settings' && <SettingsScreen />}
          </div>
        </main>
      </div>
    </div>
  )
}

export default App
