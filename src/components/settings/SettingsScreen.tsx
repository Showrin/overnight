import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { PERMISSION_MODES } from '@/lib/permissionModes'
import { TERMINAL_HOSTS, TERMINAL_HOST_LABELS } from '@/lib/terminalHost'
import { useAppStore } from '@/store/useAppStore'
import { JiraConfigForm, type JiraConfig } from './JiraConfigForm'

const SHOW_JIRA_SETTINGS = false

export function SettingsScreen() {
  const settings = useAppStore((s) => s.settings)
  const saveSettings = useAppStore((s) => s.saveSettings)
  const saveSkillFolders = useAppStore((s) => s.saveSkillFolders)
  const defaultTerminalHost = useAppStore((s) => s.defaultTerminalHost)
  const saveDefaultTerminalHost = useAppStore((s) => s.saveDefaultTerminalHost)
  const [permissionMode, setPermissionMode] = useState<string>('default')
  const [savingMode, setSavingMode] = useState(false)
  const [modeError, setModeError] = useState<string | null>(null)

  const [skillFolders, setSkillFolders] = useState<string[]>([])
  const [savingSkillFolders, setSavingSkillFolders] = useState(false)
  const [skillFoldersError, setSkillFoldersError] = useState<string | null>(null)

  const [terminalHost, setTerminalHost] = useState<string>('cmd')
  const [savingTerminalHost, setSavingTerminalHost] = useState(false)
  const [terminalHostError, setTerminalHostError] = useState<string | null>(null)

  const [jiraConfig, setJiraConfig] = useState<JiraConfig | null>(null)
  const [editingJira, setEditingJira] = useState(false)

  useEffect(() => {
    if (settings) setPermissionMode(settings.default_claude_permission_mode)
    if (settings) setSkillFolders(settings.skill_folders)
  }, [settings])

  useEffect(() => {
    setTerminalHost(defaultTerminalHost)
  }, [defaultTerminalHost])

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

  async function handleSaveTerminalHost() {
    setSavingTerminalHost(true)
    setTerminalHostError(null)
    try {
      await saveDefaultTerminalHost(terminalHost)
    } catch (e) {
      setTerminalHostError(String(e))
    } finally {
      setSavingTerminalHost(false)
    }
  }

  async function addSkillFolders() {
    const selection = await open({ directory: true, multiple: true })
    if (!selection) return
    const paths = Array.isArray(selection) ? selection : [selection]
    setSkillFolders((existing) => [...existing, ...paths.filter((p) => !existing.includes(p))])
  }

  function removeSkillFolder(path: string) {
    setSkillFolders((existing) => existing.filter((p) => p !== path))
  }

  async function handleSaveSkillFolders() {
    setSavingSkillFolders(true)
    setSkillFoldersError(null)
    try {
      await saveSkillFolders(skillFolders)
    } catch (e) {
      setSkillFoldersError(String(e))
    } finally {
      setSavingSkillFolders(false)
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

      <Card className="w-full">
        <CardHeader>
          <CardTitle>Default terminal host (Windows)</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <Select value={terminalHost} onValueChange={setTerminalHost}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {TERMINAL_HOSTS.map((host) => (
                <SelectItem key={host} value={host}>
                  {TERMINAL_HOST_LABELS[host]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <p className="text-xs text-muted-foreground">
            Which console app opens the sandbox terminal on Windows. Has no effect on macOS/Linux.
          </p>
          {terminalHostError && <p className="text-sm text-destructive">{terminalHostError}</p>}
          <Button onClick={handleSaveTerminalHost} disabled={savingTerminalHost}>
            {savingTerminalHost ? 'Saving…' : 'Save'}
          </Button>
        </CardContent>
      </Card>

      <Card className="w-full">
        <CardHeader>
          <CardTitle>Skill folders</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-2">
          <Button type="button" variant="outline" onClick={addSkillFolders}>
            Add folder(s)
          </Button>
          {skillFolders.length > 0 && (
            <ul className="flex flex-col gap-1">
              {skillFolders.map((path) => (
                <li
                  key={path}
                  className="flex items-center justify-between gap-2 rounded-lg border border-border px-2.5 py-1 text-sm"
                >
                  <span className="truncate">{path}</span>
                  <button
                    type="button"
                    onClick={() => removeSkillFolder(path)}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
          {skillFoldersError && <p className="text-sm text-destructive">{skillFoldersError}</p>}
          <Button onClick={handleSaveSkillFolders} disabled={savingSkillFolders}>
            {savingSkillFolders ? 'Saving…' : 'Save'}
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
