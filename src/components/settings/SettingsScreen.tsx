import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { NetworkPolicySettings, NetworkRuleDecision } from '@/lib/networkPolicy'
import { NETWORK_POLICY_PRESET_LABELS, NETWORK_POLICY_PRESETS } from '@/lib/networkPolicy'
import { PERMISSION_MODES } from '@/lib/permissionModes'
import { TERMINAL_HOSTS, TERMINAL_HOST_LABELS } from '@/lib/terminalHost'
import { useAppStore } from '@/store/useAppStore'
import { JiraConfigForm, type JiraConfig } from './JiraConfigForm'
import { NetworkRuleEditor } from './NetworkRuleEditor'

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

  const [networkPreset, setNetworkPreset] = useState<string>('balanced')
  const [networkPresetInitialized, setNetworkPresetInitialized] = useState(false)
  const [networkRules, setNetworkRules] = useState<NetworkPolicySettings['rules']>([])
  const [loadingNetworkPolicy, setLoadingNetworkPolicy] = useState(false)
  const [savingNetworkPreset, setSavingNetworkPreset] = useState(false)
  const [networkPolicyError, setNetworkPolicyError] = useState<string | null>(null)
  const [confirmingPresetChange, setConfirmingPresetChange] = useState(false)

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

  async function loadNetworkPolicy() {
    setLoadingNetworkPolicy(true)
    setNetworkPolicyError(null)
    try {
      const policy = await invoke<NetworkPolicySettings>('get_network_policy_settings')
      if (policy.preset) {
        setNetworkPreset(policy.preset)
        setNetworkPresetInitialized(true)
      } else {
        setNetworkPresetInitialized(false)
      }
      setNetworkRules(policy.rules)
    } catch (e) {
      setNetworkPolicyError(String(e))
    } finally {
      setLoadingNetworkPolicy(false)
    }
  }

  useEffect(() => {
    loadNetworkPolicy()
  }, [])

  async function handleSaveNetworkPreset() {
    setSavingNetworkPreset(true)
    setNetworkPolicyError(null)
    try {
      await invoke('save_default_network_policy_preset', { preset: networkPreset })
      setNetworkPresetInitialized(true)
    } catch (e) {
      setNetworkPolicyError(String(e))
    } finally {
      setSavingNetworkPreset(false)
    }
  }

  // A first-time preset choice just initializes sbx's policy — nothing to
  // confirm. Changing an already-initialized preset resets sbx's network
  // daemon and stops every currently running sandbox, so that path always
  // needs an explicit confirmation first.
  function handleApplyNetworkPresetClick() {
    if (networkPresetInitialized) {
      setConfirmingPresetChange(true)
    } else {
      handleSaveNetworkPreset()
    }
  }

  async function confirmNetworkPresetChange() {
    setConfirmingPresetChange(false)
    await handleSaveNetworkPreset()
  }

  async function addGlobalNetworkRule(decision: NetworkRuleDecision, host: string) {
    await invoke('add_network_rule', { decision, host })
    await loadNetworkPolicy()
  }

  async function removeGlobalNetworkRule(host: string) {
    await invoke('remove_network_rule', { host })
    await loadNetworkPolicy()
  }

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
          <CardTitle>Network Policy</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-col gap-2">
            <Select value={networkPreset} onValueChange={setNetworkPreset}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {NETWORK_POLICY_PRESETS.map((preset) => (
                  <SelectItem key={preset} value={preset}>
                    {NETWORK_POLICY_PRESET_LABELS[preset]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {networkPresetInitialized
                ? "Changing this may interrupt currently running sandboxes — sbx applies network policy presets machine-wide."
                : "sbx has no network policy configured on this machine yet. Choose a preset to enable creating sandboxes."}
            </p>
            {networkPolicyError && <p className="text-sm text-destructive">{networkPolicyError}</p>}
            <Button onClick={handleApplyNetworkPresetClick} disabled={savingNetworkPreset}>
              {savingNetworkPreset ? 'Applying…' : 'Apply'}
            </Button>
          </div>
          <div className="flex flex-col gap-2 border-t border-border pt-3">
            <span className="text-xs font-medium text-foreground">Custom rules</span>
            {loadingNetworkPolicy ? (
              <p className="text-xs text-muted-foreground">Loading…</p>
            ) : (
              <NetworkRuleEditor rules={networkRules} onAdd={addGlobalNetworkRule} onRemove={removeGlobalNetworkRule} />
            )}
          </div>
        </CardContent>
      </Card>

      <Dialog open={confirmingPresetChange} onOpenChange={setConfirmingPresetChange}>
        <DialogContent title="Change network policy?">
          <Card className="w-full">
            <CardHeader>
              <CardTitle>Change network policy?</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <p className="text-sm text-muted-foreground">
                Changing the network policy resets sbx's network daemon and stops every currently
                running sandbox on this machine. This can't be undone.
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setConfirmingPresetChange(false)}>
                  Cancel
                </Button>
                <Button variant="destructive" onClick={confirmNetworkPresetChange} disabled={savingNetworkPreset}>
                  {savingNetworkPreset ? 'Applying…' : 'Reset & apply'}
                </Button>
              </div>
            </CardContent>
          </Card>
        </DialogContent>
      </Dialog>

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
