import { useEffect, useState } from 'react'
import { Titlebar } from '@/components/titlebar'
import { Sidebar, type Screen } from '@/components/sidebar'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'
import { ProjectsScreen } from '@/components/projects/ProjectsScreen'
import { SandboxesScreen } from '@/components/sandboxes/SandboxesScreen'
import { SettingsScreen } from '@/components/settings/SettingsScreen'
import { initAppStore } from '@/store/useAppStore'

function App() {
  const [screen, setScreen] = useState<Screen>('projects')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)

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
            {screen === 'sandboxes' && <SandboxesScreen />}
            {screen === 'settings' && <SettingsScreen />}
          </div>
        </main>
      </div>
    </div>
  )
}

export default App
