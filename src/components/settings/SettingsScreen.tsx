import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { NetworkPolicySettings, NetworkRuleDecision } from '@/lib/networkPolicy'
import type { Route, SettingsTab } from '@/lib/router'
import { useAppStore } from '@/store/useAppStore'
import type { JiraConfig } from './JiraConfigForm'
import { SettingsActions, type TabActions, UnsavedChangesNotice } from './SettingsLayout'
import { BackupsTab } from './tabs/BackupsTab'
import { GeneralTab } from './tabs/GeneralTab'
import { IntegrationsTab } from './tabs/IntegrationsTab'
import { NetworkTab } from './tabs/NetworkTab'

const SHOW_JIRA_SETTINGS = false

function sameFolders(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((path, i) => path === b[i])
}

interface SettingsScreenProps {
  settingsTab: SettingsTab
  navigate: (route: Route) => void
}

export function SettingsScreen({ settingsTab, navigate }: SettingsScreenProps) {
  const settings = useAppStore((s) => s.settings)
  const saveSettings = useAppStore((s) => s.saveSettings)
  const saveSkillFolders = useAppStore((s) => s.saveSkillFolders)
  const defaultTerminalHost = useAppStore((s) => s.defaultTerminalHost)
  const saveDefaultTerminalHost = useAppStore((s) => s.saveDefaultTerminalHost)
  const backupIntervalMinutes = useAppStore((s) => s.backupIntervalMinutes)
  const saveBackupIntervalMinutes = useAppStore((s) => s.saveBackupIntervalMinutes)
  const autoBackupEnabled = useAppStore((s) => s.autoBackupEnabled)
  const saveAutoBackupEnabled = useAppStore((s) => s.saveAutoBackupEnabled)

  const savedPermissionMode = settings?.default_claude_permission_mode ?? 'default'
  const savedSkillFolders = settings?.skill_folders ?? []

  const [permissionMode, setPermissionMode] = useState<string>('default')
  const [terminalHost, setTerminalHost] = useState<string>('cmd')
  const [skillFolders, setSkillFolders] = useState<string[]>([])
  const [savingGeneral, setSavingGeneral] = useState(false)
  const [generalError, setGeneralError] = useState<string | null>(null)

  const [autoBackup, setAutoBackup] = useState(true)
  const [backupInterval, setBackupInterval] = useState<number>(15)
  const [savingBackups, setSavingBackups] = useState(false)
  const [backupsError, setBackupsError] = useState<string | null>(null)

  const [networkPreset, setNetworkPreset] = useState<string>('balanced')
  const [savedNetworkPreset, setSavedNetworkPreset] = useState<string | null>(null)
  const [networkRules, setNetworkRules] = useState<NetworkPolicySettings['rules']>([])
  const [loadingNetworkPolicy, setLoadingNetworkPolicy] = useState(false)
  const [savingNetwork, setSavingNetwork] = useState(false)
  const [networkError, setNetworkError] = useState<string | null>(null)
  const [confirmingPresetChange, setConfirmingPresetChange] = useState(false)

  const [jiraConfig, setJiraConfig] = useState<JiraConfig | null>(null)
  const [editingJira, setEditingJira] = useState(false)

  useEffect(() => {
    if (settings) {
      setPermissionMode(settings.default_claude_permission_mode)
      setSkillFolders(settings.skill_folders)
    }
  }, [settings])

  useEffect(() => {
    setTerminalHost(defaultTerminalHost)
  }, [defaultTerminalHost])

  useEffect(() => {
    setBackupInterval(backupIntervalMinutes)
  }, [backupIntervalMinutes])

  useEffect(() => {
    setAutoBackup(autoBackupEnabled)
  }, [autoBackupEnabled])

  async function loadJiraConfig() {
    const c = await invoke<JiraConfig>('get_jira_config')
    setJiraConfig(c)
  }

  useEffect(() => {
    loadJiraConfig()
  }, [])

  async function loadNetworkPolicy() {
    setLoadingNetworkPolicy(true)
    setNetworkError(null)
    try {
      const policy = await invoke<NetworkPolicySettings>('get_network_policy_settings')
      if (policy.preset) {
        setNetworkPreset(policy.preset)
        setSavedNetworkPreset(policy.preset)
      } else {
        setSavedNetworkPreset(null)
      }
      setNetworkRules(policy.rules)
    } catch (e) {
      setNetworkError(String(e))
    } finally {
      setLoadingNetworkPolicy(false)
    }
  }

  useEffect(() => {
    loadNetworkPolicy()
  }, [])

  async function addSkillFolders() {
    const selection = await open({ directory: true, multiple: true })
    if (!selection) return
    const paths = Array.isArray(selection) ? selection : [selection]
    setSkillFolders((existing) => [...existing, ...paths.filter((p) => !existing.includes(p))])
  }

  function removeSkillFolder(path: string) {
    setSkillFolders((existing) => existing.filter((p) => p !== path))
  }

  const generalDirty =
    permissionMode !== savedPermissionMode ||
    terminalHost !== defaultTerminalHost ||
    !sameFolders(skillFolders, savedSkillFolders)

  async function saveGeneral() {
    setSavingGeneral(true)
    setGeneralError(null)
    try {
      if (permissionMode !== savedPermissionMode) await saveSettings(permissionMode)
      if (!sameFolders(skillFolders, savedSkillFolders)) await saveSkillFolders(skillFolders)
      if (terminalHost !== defaultTerminalHost) await saveDefaultTerminalHost(terminalHost)
    } catch (e) {
      setGeneralError(String(e))
    } finally {
      setSavingGeneral(false)
    }
  }

  function resetGeneral() {
    setPermissionMode(savedPermissionMode)
    setTerminalHost(defaultTerminalHost)
    setSkillFolders(savedSkillFolders)
    setGeneralError(null)
  }

  const backupsDirty = autoBackup !== autoBackupEnabled || backupInterval !== backupIntervalMinutes

  async function saveBackups() {
    setSavingBackups(true)
    setBackupsError(null)
    try {
      if (autoBackup !== autoBackupEnabled) await saveAutoBackupEnabled(autoBackup)
      if (backupInterval !== backupIntervalMinutes) await saveBackupIntervalMinutes(backupInterval)
    } catch (e) {
      setBackupsError(String(e))
    } finally {
      setSavingBackups(false)
    }
  }

  function resetBackups() {
    setAutoBackup(autoBackupEnabled)
    setBackupInterval(backupIntervalMinutes)
    setBackupsError(null)
  }

  const networkDirty = savedNetworkPreset !== null && networkPreset !== savedNetworkPreset

  async function applyNetworkPreset() {
    setSavingNetwork(true)
    setNetworkError(null)
    try {
      await invoke('save_default_network_policy_preset', { preset: networkPreset })
      setSavedNetworkPreset(networkPreset)
    } catch (e) {
      setNetworkError(String(e))
    } finally {
      setSavingNetwork(false)
    }
  }

  // A first-time preset choice just initializes sbx's policy — nothing to
  // confirm. Changing an already-initialized preset resets sbx's network
  // daemon and stops every currently running sandbox, so that path always
  // needs an explicit confirmation first.
  function saveNetwork() {
    if (savedNetworkPreset !== null) {
      setConfirmingPresetChange(true)
    } else {
      applyNetworkPreset()
    }
  }

  async function confirmNetworkPresetChange() {
    setConfirmingPresetChange(false)
    await applyNetworkPreset()
  }

  function resetNetwork() {
    setNetworkPreset(savedNetworkPreset ?? 'balanced')
    setNetworkError(null)
  }

  async function addGlobalNetworkRule(decision: NetworkRuleDecision, host: string) {
    await invoke('add_network_rule', { decision, host })
    await loadNetworkPolicy()
  }

  async function removeGlobalNetworkRule(host: string) {
    await invoke('remove_network_rule', { host })
    await loadNetworkPolicy()
  }

  const noop = () => {}
  const tabActions: Record<SettingsTab, TabActions> = {
    general: {
      dirty: generalDirty,
      canSave: generalDirty,
      saving: savingGeneral,
      error: generalError,
      onSave: saveGeneral,
      onCancel: resetGeneral,
    },
    backups: {
      dirty: backupsDirty,
      canSave: backupsDirty,
      saving: savingBackups,
      error: backupsError,
      onSave: saveBackups,
      onCancel: resetBackups,
    },
    network: {
      dirty: networkDirty,
      canSave: networkDirty || savedNetworkPreset === null,
      saving: savingNetwork,
      error: networkError,
      onSave: saveNetwork,
      onCancel: resetNetwork,
    },
    integrations: { dirty: false, canSave: false, saving: false, error: null, onSave: noop, onCancel: noop },
  }

  const activeActions = tabActions[settingsTab]

  return (
    <Tabs
      value={settingsTab}
      onValueChange={(value) => navigate({ screen: 'settings', settingsTab: value as SettingsTab })}
      className="flex h-full min-h-0 w-full flex-col gap-4"
    >
      <div className="flex shrink-0 flex-col gap-4">
        <div className="flex items-center justify-between gap-4">
          <h1 className="text-lg font-medium">Settings</h1>
          {settingsTab !== 'integrations' && <SettingsActions {...activeActions} />}
        </div>

        <TabsList>
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="backups">Backups</TabsTrigger>
          <TabsTrigger value="network">Network</TabsTrigger>
          {SHOW_JIRA_SETTINGS && <TabsTrigger value="integrations">Integrations</TabsTrigger>}
        </TabsList>

        {activeActions.dirty && <UnsavedChangesNotice />}
        {activeActions.error && <p className="text-sm text-destructive">{activeActions.error}</p>}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <TabsContent value="general">
          <GeneralTab
            permissionMode={permissionMode}
            setPermissionMode={setPermissionMode}
            terminalHost={terminalHost}
            setTerminalHost={setTerminalHost}
            skillFolders={skillFolders}
            onAddSkillFolders={addSkillFolders}
            onRemoveSkillFolder={removeSkillFolder}
          />
        </TabsContent>

        <TabsContent value="backups">
          <BackupsTab
            autoBackup={autoBackup}
            setAutoBackup={setAutoBackup}
            backupInterval={backupInterval}
            setBackupInterval={setBackupInterval}
          />
        </TabsContent>

        <TabsContent value="network">
          <NetworkTab
            networkPreset={networkPreset}
            setNetworkPreset={setNetworkPreset}
            networkInitialized={savedNetworkPreset !== null}
            savingNetwork={savingNetwork}
            loadingNetworkPolicy={loadingNetworkPolicy}
            networkRules={networkRules}
            onAddNetworkRule={addGlobalNetworkRule}
            onRemoveNetworkRule={removeGlobalNetworkRule}
            confirmingPresetChange={confirmingPresetChange}
            setConfirmingPresetChange={setConfirmingPresetChange}
            onConfirmNetworkPresetChange={confirmNetworkPresetChange}
          />
        </TabsContent>

        {SHOW_JIRA_SETTINGS && (
          <TabsContent value="integrations">
            <IntegrationsTab
              jiraConfig={jiraConfig}
              editingJira={editingJira}
              setEditingJira={setEditingJira}
              onJiraSaved={loadJiraConfig}
            />
          </TabsContent>
        )}
      </div>
    </Tabs>
  )
}
