import { Titlebar } from '@/components/titlebar'
import { JiraIssueList } from '@/components/dashboard/JiraIssueList'

function App() {
  return (
    <div className="flex h-screen flex-col bg-background">
      <Titlebar />
      <main className="flex flex-1 items-center justify-center">
        <JiraIssueList />
      </main>
    </div>
  )
}

export default App
