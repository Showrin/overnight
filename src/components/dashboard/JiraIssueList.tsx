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

const inputClassName =
  'h-8 rounded-lg border border-border bg-background px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50'

function JiraConfigForm({
  initial,
  onSaved,
  onCancel,
}: {
  initial: JiraConfig | null
  onSaved: () => void
  onCancel?: () => void
}) {
  const [site, setSite] = useState(initial?.site ?? '')
  const [email, setEmail] = useState(initial?.email ?? '')
  const [jql, setJql] = useState(initial?.jql ?? '')
  const [apiToken, setApiToken] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSave() {
    setSaving(true)
    setError(null)
    try {
      await invoke('save_jira_config', {
        site,
        email,
        jql: jql.trim() ? jql : null,
        apiToken,
      })
      await onSaved()
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card className="w-96">
      <CardHeader>
        <CardTitle>Connect Jira</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <input
          className={inputClassName}
          placeholder="Site (https://your-team.atlassian.net)"
          value={site}
          onChange={(e) => setSite(e.target.value)}
        />
        <input
          className={inputClassName}
          placeholder="Email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        <input
          className={inputClassName}
          placeholder="JQL (defaults to your assigned issues)"
          value={jql}
          onChange={(e) => setJql(e.target.value)}
        />
        <input
          className={inputClassName}
          type="password"
          placeholder="API token"
          value={apiToken}
          onChange={(e) => setApiToken(e.target.value)}
        />
        {error && <p className="text-sm text-destructive">{error}</p>}
        <div className="flex gap-2">
          <Button
            className="flex-1"
            onClick={handleSave}
            disabled={saving || !site || !email || !apiToken}
          >
            {saving ? 'Saving…' : 'Save'}
          </Button>
          {onCancel && (
            <Button variant="outline" onClick={onCancel} disabled={saving}>
              Cancel
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  )
}

export function JiraIssueList() {
  const [config, setConfig] = useState<JiraConfig | null>(null)
  const [issues, setIssues] = useState<JiraIssue[]>([])
  const [syncing, setSyncing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [editing, setEditing] = useState(false)

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

  if (!config.has_token || editing) {
    return (
      <JiraConfigForm
        initial={config}
        onSaved={() => {
          setEditing(false)
          return loadConfig().then(loadIssues)
        }}
        onCancel={config.has_token ? () => setEditing(false) : undefined}
      />
    )
  }

  return (
    <Card className="w-96">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>Jira issues</CardTitle>
        <div className="flex gap-2">
          <Button size="sm" variant="outline" onClick={() => setEditing(true)}>
            Edit
          </Button>
          <Button size="sm" onClick={handleSync} disabled={syncing}>
            {syncing ? 'Syncing…' : 'Sync now'}
          </Button>
        </div>
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
