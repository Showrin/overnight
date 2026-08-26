import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { PERMISSION_MODES } from '@/lib/permissionModes'
import { useAppStore } from '@/store/useAppStore'
import { JiraConfigForm, type JiraConfig } from './JiraConfigForm'

const SHOW_JIRA_SETTINGS = false

export function SettingsScreen() {
  const settings = useAppStore((s) => s.settings)
  const saveSettings = useAppStore((s) => s.saveSettings)
  const [permissionMode, setPermissionMode] = useState<string>('default')
  const [savingMode, setSavingMode] = useState(false)
  const [modeError, setModeError] = useState<string | null>(null)

  const [jiraConfig, setJiraConfig] = useState<JiraConfig | null>(null)
  const [editingJira, setEditingJira] = useState(false)

  useEffect(() => {
    if (settings) setPermissionMode(settings.default_claude_permission_mode)
  }, [settings])

  async function loadJiraConfig() {
    const c = await invoke<JiraConfig>('get_jira_config')
    setJiraConfig(c)
  }

  useEffect(() => {
    loadJiraConfig()
  }, [])

  async function handleSaveMode() {
    setSavingMode(true)
    setModeError(null)
    try {
      await saveSettings(permissionMode)
    } catch (e) {
      setModeError(String(e))
    } finally {
      setSavingMode(false)
    }
  }

  return (
    <div className="flex w-full flex-col gap-4">
      <h1 className="text-lg font-medium">Settings</h1>

      <Card className="w-full">
        <CardHeader>
          <CardTitle>Default Claude permission mode</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <Select value={permissionMode} onValueChange={setPermissionMode}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PERMISSION_MODES.map((mode) => (
                <SelectItem key={mode} value={mode}>
                  {mode}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {modeError && <p className="text-sm text-destructive">{modeError}</p>}
          <Button onClick={handleSaveMode} disabled={savingMode}>
            {savingMode ? 'Saving…' : 'Save'}
          </Button>
        </CardContent>
      </Card>

      {SHOW_JIRA_SETTINGS && jiraConfig && (!jiraConfig.has_token || editingJira) ? (
        <JiraConfigForm
          initial={jiraConfig}
          onSaved={() => {
            setEditingJira(false)
            return loadJiraConfig()
          }}
          onCancel={jiraConfig.has_token ? () => setEditingJira(false) : undefined}
        />
      ) : SHOW_JIRA_SETTINGS && jiraConfig ? (
        <Card className="w-full">
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
