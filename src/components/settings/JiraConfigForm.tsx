import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

export interface JiraConfig {
  site: string | null
  email: string | null
  jql: string | null
  has_token: boolean
}

const inputClassName =
  'h-8 rounded-lg border border-border bg-background px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50'

export function JiraConfigForm({
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
