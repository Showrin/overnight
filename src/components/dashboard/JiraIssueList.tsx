import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

interface JiraConfig {
  site: string | null
  email: string | null
  jql: string | null
  has_token: boolean
}

interface JiraIssue {
  id: string
  key: string
  summary: string
  status: string
  issue_type: string | null
  priority: string | null
  assignee: string | null
  url: string
}

export function JiraIssueList() {
  const [config, setConfig] = useState<JiraConfig | null>(null)
  const [issues, setIssues] = useState<JiraIssue[]>([])
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function loadConfig() {
    const c = await invoke<JiraConfig>('get_jira_config')
    setConfig(c)
    return c
  }

  async function loadIssues() {
    const list = await invoke<JiraIssue[]>('list_jira_issues')
    setIssues(list)
  }

  useEffect(() => {
    loadConfig().then((c) => {
      if (c.has_token) loadIssues()
    })
  }, [])

  async function handleSync() {
    setSyncing(true)
    setError(null)
    try {
      await invoke('sync_jira_issues')
      await loadIssues()
    } catch (e) {
      setError(String(e))
    } finally {
      setSyncing(false)
    }
  }

  if (!config) return null

  if (!config.has_token) {
    return (
      <Card className="w-full">
        <CardHeader>
          <CardTitle>Jira issues</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            Connect Jira in Settings to see issues.
          </p>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card className="w-full">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>Jira issues</CardTitle>
        <Button size="sm" onClick={handleSync} disabled={syncing}>
          {syncing ? 'Syncing…' : 'Sync now'}
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {error && <p className="text-sm text-destructive">{error}</p>}
        {issues.length === 0 && !error && (
          <p className="text-sm text-muted-foreground">
            No issues cached yet — click Sync now.
          </p>
        )}
        {issues.map((issue) => (
          <a
            key={issue.id}
            href={issue.url}
            target="_blank"
            rel="noreferrer"
            className="flex flex-col gap-1 rounded-lg border border-border p-2.5 text-sm hover:bg-muted"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="font-medium">{issue.key}</span>
              <Badge variant="outline">{issue.status}</Badge>
            </div>
            <span className="text-muted-foreground">{issue.summary}</span>
          </a>
        ))}
      </CardContent>
    </Card>
  )
}
