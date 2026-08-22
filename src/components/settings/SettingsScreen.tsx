import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { JiraConfigForm, type JiraConfig } from './JiraConfigForm'

interface AppSettings {
  default_claude_permission_mode: string
}

const PERMISSION_MODES = ['plan', 'default', 'acceptEdits', 'bypassPermissions'] as const

const selectClassName =
  'h-8 rounded-lg border border-border bg-background px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50'

export function SettingsScreen() {
  const [permissionMode, setPermissionMode] = useState<string>('default')
  const [savingMode, setSavingMode] = useState(false)
  const [modeError, setModeError] = useState<string | null>(null)

  const [jiraConfig, setJiraConfig] = useState<JiraConfig | null>(null)
  const [editingJira, setEditingJira] = useState(false)

  async function loadSettings() {
    const s = await invoke<AppSettings>('get_settings')
    setPermissionMode(s.default_claude_permission_mode)
  }

  async function loadJiraConfig() {
    const c = await invoke<JiraConfig>('get_jira_config')
    setJiraConfig(c)
  }

  useEffect(() => {
    loadSettings()
    loadJiraConfig()
  }, [])

  async function handleSaveMode() {
    setSavingMode(true)
    setModeError(null)
    try {
      await invoke('save_settings', { defaultClaudePermissionMode: permissionMode })
    } catch (e) {
      setModeError(String(e))
    } finally {
      setSavingMode(false)
    }
  }

  return (
    <div className="flex w-full max-w-2xl flex-col gap-4 p-6">
      <h1 className="text-lg font-medium">Settings</h1>

      <Card className="w-96">
        <CardHeader>
          <CardTitle>Default Claude permission mode</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <select
            className={selectClassName}
            value={permissionMode}
            onChange={(e) => setPermissionMode(e.target.value)}
          >
            {PERMISSION_MODES.map((mode) => (
              <option key={mode} value={mode}>
                {mode}
              </option>
            ))}
          </select>
          {modeError && <p className="text-sm text-destructive">{modeError}</p>}
          <Button onClick={handleSaveMode} disabled={savingMode}>
            {savingMode ? 'Saving…' : 'Save'}
          </Button>
        </CardContent>
      </Card>

      {jiraConfig && (!jiraConfig.has_token || editingJira) ? (
        <JiraConfigForm
          initial={jiraConfig}
          onSaved={() => {
            setEditingJira(false)
            return loadJiraConfig()
          }}
          onCancel={jiraConfig.has_token ? () => setEditingJira(false) : undefined}
        />
      ) : jiraConfig ? (
        <Card className="w-96">
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle>Jira</CardTitle>
            <Button size="sm" variant="outline" onClick={() => setEditingJira(true)}>
              Edit
            </Button>
          </CardHeader>
          <CardContent className="text-sm text-muted-foreground">Connected to {jiraConfig.site}</CardContent>
        </Card>
      ) : null}
    </div>
  )
}
