import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { JiraConfigForm, type JiraConfig } from '../JiraConfigForm'

interface IntegrationsTabProps {
  jiraConfig: JiraConfig | null
  editingJira: boolean
  setEditingJira: (editing: boolean) => void
  onJiraSaved: () => Promise<void>
}

export function IntegrationsTab({ jiraConfig, editingJira, setEditingJira, onJiraSaved }: IntegrationsTabProps) {
  if (!jiraConfig) return null

  if (!jiraConfig.has_token || editingJira) {
    return (
      <JiraConfigForm
        initial={jiraConfig}
        onSaved={() => {
          setEditingJira(false)
          return onJiraSaved()
        }}
        onCancel={jiraConfig.has_token ? () => setEditingJira(false) : undefined}
      />
    )
  }

  return (
    <Card className="w-full">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>Jira</CardTitle>
        <Button size="sm" variant="outline" onClick={() => setEditingJira(true)}>
          Edit
        </Button>
      </CardHeader>
      <CardContent className="text-sm text-muted-foreground">Connected to {jiraConfig.site}</CardContent>
    </Card>
  )
}
