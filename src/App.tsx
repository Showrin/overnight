import { useState } from 'react'
import { Titlebar } from '@/components/titlebar'
import { Sidebar, type Screen } from '@/components/sidebar'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'

function App() {
  const [screen, setScreen] = useState<Screen>('dashboard')

  return (
    <div className="flex h-screen flex-col bg-background">
      <Titlebar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar current={screen} onNavigate={setScreen} />
        <main className="flex flex-1 items-center justify-center overflow-auto">
          {screen === 'dashboard' && <JiraIssueList />}
        </main>
      </div>
    </div>
  )
}

export default App
