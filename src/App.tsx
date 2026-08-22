import { useState } from 'react'
import { Titlebar } from '@/components/titlebar'
import { Sidebar, type Screen } from '@/components/sidebar'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'
import { ProjectsScreen } from '@/components/projects/ProjectsScreen'
import { SandboxesScreen } from '@/components/sandboxes/SandboxesScreen'
import { SettingsScreen } from '@/components/settings/SettingsScreen'

function App() {
  const [screen, setScreen] = useState<Screen>('dashboard')

  return (
    <div className="flex h-screen flex-col bg-background">
      <Titlebar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar current={screen} onNavigate={setScreen} />
        <main className="flex flex-1 overflow-auto">
          {screen === 'dashboard' && (
            <div className="flex flex-1 items-center justify-center">
              <JiraIssueList />
            </div>
          )}
          {screen === 'projects' && <ProjectsScreen />}
          {screen === 'sandboxes' && <SandboxesScreen />}
          {screen === 'settings' && <SettingsScreen />}
        </main>
      </div>
    </div>
  )
}

export default App
