import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { EnvVar } from '@/lib/envVars'
import type { NetworkPolicySettings, NetworkRuleDecision } from '@/lib/networkPolicy'
import type { Secret } from '@/lib/secrets'
import type { Route, SettingsTab } from '@/lib/router'
import { useAppStore } from '@/store/useAppStore'
import type { JiraConfig } from './JiraConfigForm'
import { EnvVarsSavedDialog } from './EnvVarsSavedDialog'
import { SettingsActions, type TabActions, UnsavedChangesNotice } from './SettingsLayout'
import { BackupsTab } from './tabs/BackupsTab'
import { CredentialsTab } from './tabs/CredentialsTab'
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
  const projects = useAppStore((s) => s.projects)
  const saveSettings = useAppStore((s) => s.saveSettings)
  const saveSkillFolders = useAppStore((s) => s.saveSkillFolders)
  const defaultTerminalHost = useAppStore((s) => s.defaultTerminalHost)
  const saveDefaultTerminalHost = useAppStore((s) => s.saveDefaultTerminalHost)
  const defaultAgent = useAppStore((s) => s.defaultAgent)
  const saveDefaultAgent = useAppStore((s) => s.saveDefaultAgent)
  const backupIntervalMinutes = useAppStore((s) => s.backupIntervalMinutes)
  const saveBackupIntervalMinutes = useAppStore((s) => s.saveBackupIntervalMinutes)
  const autoBackupEnabled = useAppStore((s) => s.autoBackupEnabled)
  const saveAutoBackupEnabled = useAppStore((s) => s.saveAutoBackupEnabled)
  const loadNetworkPolicyPreset = useAppStore((s) => s.loadNetworkPolicyPreset)
  const highlightNetworkPreset = useAppStore((s) => s.highlightNetworkPreset)
  const setHighlightNetworkPreset = useAppStore((s) => s.setHighlightNetworkPreset)

  const savedPermissionMode = settings?.default_claude_permission_mode ?? 'default'
  const savedCodexPermissionMode = settings?.default_codex_permission_mode ?? 'on-request'
  const savedSkillFolders = settings?.skill_folders ?? []

  const [permissionMode, setPermissionMode] = useState<string>('default')
  const [codexPermissionMode, setCodexPermissionMode] = useState<string>('on-request')
  const [localDefaultAgent, setLocalDefaultAgent] = useState<string>('claude')
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

  const [globalEnvVars, setGlobalEnvVars] = useState<EnvVar[]>([])
  const [savedGlobalEnvVars, setSavedGlobalEnvVars] = useState<EnvVar[]>([])
  const [savingGlobalEnv, setSavingGlobalEnv] = useState(false)
  const [globalEnvError, setGlobalEnvError] = useState<string | null>(null)

  const [selectedEnvProjectId, setSelectedEnvProjectId] = useState<string | null>(null)
  const [projectEnvVars, setProjectEnvVars] = useState<EnvVar[]>([])
  const [savedProjectEnvVars, setSavedProjectEnvVars] = useState<EnvVar[]>([])
  const [loadingProjectEnvVars, setLoadingProjectEnvVars] = useState(false)
  const [savingProjectEnv, setSavingProjectEnv] = useState(false)
  const [projectEnvError, setProjectEnvError] = useState<string | null>(null)

  const [globalSecrets, setGlobalSecrets] = useState<Secret[]>([])
  const [savedGlobalSecrets, setSavedGlobalSecrets] = useState<Secret[]>([])
  const [savingGlobalSecrets, setSavingGlobalSecrets] = useState(false)
  const [globalSecretsError, setGlobalSecretsError] = useState<string | null>(null)

  const [selectedSecretProjectId, setSelectedSecretProjectId] = useState<string | null>(null)
  const [projectSecrets, setProjectSecrets] = useState<Secret[]>([])
  const [savedProjectSecrets, setSavedProjectSecrets] = useState<Secret[]>([])
  const [loadingProjectSecrets, setLoadingProjectSecrets] = useState(false)
  const [savingProjectSecrets, setSavingProjectSecrets] = useState(false)
  const [projectSecretsError, setProjectSecretsError] = useState<string | null>(null)

  const [credentialsSavedDialogOpen, setCredentialsSavedDialogOpen] = useState(false)

  useEffect(() => {
    if (settings) {
      setPermissionMode(settings.default_claude_permission_mode)
      setCodexPermissionMode(savedCodexPermissionMode)
      setSkillFolders(settings.skill_folders)
    }
  }, [settings, savedCodexPermissionMode])

  useEffect(() => {
    setTerminalHost(defaultTerminalHost)
  }, [defaultTerminalHost])

  useEffect(() => {
    setLocalDefaultAgent(defaultAgent)
  }, [defaultAgent])

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

  async function loadGlobalEnvVars() {
    const vars = await invoke<EnvVar[]>('get_global_env_vars')
    setGlobalEnvVars(vars)
    setSavedGlobalEnvVars(vars)
  }

  useEffect(() => {
    loadGlobalEnvVars()
  }, [])

  useEffect(() => {
    if (!selectedEnvProjectId) return
    setLoadingProjectEnvVars(true)
    invoke<EnvVar[]>('get_project_env_vars', { projectId: selectedEnvProjectId })
      .then((vars) => {
        setProjectEnvVars(vars)
        setSavedProjectEnvVars(vars)
      })
      .catch((e) => setProjectEnvError(String(e)))
      .finally(() => setLoadingProjectEnvVars(false))
  }, [selectedEnvProjectId])

  async function loadGlobalSecrets() {
    const secrets = await invoke<Secret[]>('get_global_secrets')
    setGlobalSecrets(secrets)
    setSavedGlobalSecrets(secrets)
  }

  useEffect(() => {
    loadGlobalSecrets()
  }, [])

  useEffect(() => {
    if (!selectedSecretProjectId) return
    setLoadingProjectSecrets(true)
    invoke<Secret[]>('get_project_secrets', { projectId: selectedSecretProjectId })
      .then((secrets) => {
        setProjectSecrets(secrets)
        setSavedProjectSecrets(secrets)
      })
      .catch((e) => setProjectSecretsError(String(e)))
      .finally(() => setLoadingProjectSecrets(false))
  }, [selectedSecretProjectId])

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
    codexPermissionMode !== savedCodexPermissionMode ||
    localDefaultAgent !== defaultAgent ||
    terminalHost !== defaultTerminalHost ||
    !sameFolders(skillFolders, savedSkillFolders)

  async function saveGeneral() {
    setSavingGeneral(true)
    setGeneralError(null)
    try {
      if (permissionMode !== savedPermissionMode || codexPermissionMode !== savedCodexPermissionMode) {
        await saveSettings(permissionMode, codexPermissionMode)
      }
      if (localDefaultAgent !== defaultAgent) await saveDefaultAgent(localDefaultAgent)
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
    setCodexPermissionMode(savedCodexPermissionMode)
    setLocalDefaultAgent(defaultAgent)
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
      await loadNetworkPolicyPreset()
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

  function sameEnvVars(a: EnvVar[], b: EnvVar[]): boolean {
    return a.length === b.length && a.every((v, i) => v.key === b[i].key && v.value === b[i].value)
  }

  const globalEnvDirty = !sameEnvVars(globalEnvVars, savedGlobalEnvVars)

  async function saveGlobalEnv(): Promise<boolean> {
    setSavingGlobalEnv(true)
    setGlobalEnvError(null)
    try {
      await invoke('save_global_env_vars', { vars: globalEnvVars })
      setSavedGlobalEnvVars(globalEnvVars)
      return true
    } catch (e) {
      setGlobalEnvError(String(e))
      return false
    } finally {
      setSavingGlobalEnv(false)
    }
  }

  function resetGlobalEnv() {
    setGlobalEnvVars(savedGlobalEnvVars)
    setGlobalEnvError(null)
  }

  const projectEnvDirty = selectedEnvProjectId !== null && !sameEnvVars(projectEnvVars, savedProjectEnvVars)

  async function saveProjectEnv(): Promise<boolean> {
    if (!selectedEnvProjectId) return true
    setSavingProjectEnv(true)
    setProjectEnvError(null)
    try {
      await invoke('save_project_env_vars', { projectId: selectedEnvProjectId, vars: projectEnvVars })
      setSavedProjectEnvVars(projectEnvVars)
      return true
    } catch (e) {
      setProjectEnvError(String(e))
      return false
    } finally {
      setSavingProjectEnv(false)
    }
  }

  function resetProjectEnv() {
    setProjectEnvVars(savedProjectEnvVars)
    setProjectEnvError(null)
  }

  function sameSecrets(a: Secret[], b: Secret[]): boolean {
    return a.length === b.length && a.every((v, i) => JSON.stringify(v) === JSON.stringify(b[i]))
  }

  const globalSecretsDirty = !sameSecrets(globalSecrets, savedGlobalSecrets)

  async function saveGlobalSecrets(): Promise<boolean> {
    setSavingGlobalSecrets(true)
    setGlobalSecretsError(null)
    try {
      await invoke('save_global_secrets', { secrets: globalSecrets })
      setSavedGlobalSecrets(globalSecrets)
      return true
    } catch (e) {
      setGlobalSecretsError(String(e))
      return false
    } finally {
      setSavingGlobalSecrets(false)
    }
  }

  function resetGlobalSecrets() {
    setGlobalSecrets(savedGlobalSecrets)
    setGlobalSecretsError(null)
  }

  const projectSecretsDirty = selectedSecretProjectId !== null && !sameSecrets(projectSecrets, savedProjectSecrets)

  async function saveProjectSecrets(): Promise<boolean> {
    if (!selectedSecretProjectId) return true
    setSavingProjectSecrets(true)
    setProjectSecretsError(null)
    try {
      await invoke('save_project_secrets', { projectId: selectedSecretProjectId, secrets: projectSecrets })
      setSavedProjectSecrets(projectSecrets)
      return true
    } catch (e) {
      setProjectSecretsError(String(e))
      return false
    } finally {
      setSavingProjectSecrets(false)
    }
  }

  function resetProjectSecrets() {
    setProjectSecrets(savedProjectSecrets)
    setProjectSecretsError(null)
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
    credentials: {
      dirty: globalEnvDirty || projectEnvDirty || globalSecretsDirty || projectSecretsDirty,
      canSave: globalEnvDirty || projectEnvDirty || globalSecretsDirty || projectSecretsDirty,
      saving: savingGlobalEnv || savingProjectEnv || savingGlobalSecrets || savingProjectSecrets,
      error: globalEnvError ?? projectEnvError ?? globalSecretsError ?? projectSecretsError,
      onSave: async () => {
        let ok = true
        if (globalEnvDirty) ok = (await saveGlobalEnv()) && ok
        if (projectEnvDirty) ok = (await saveProjectEnv()) && ok
        if (globalSecretsDirty) ok = (await saveGlobalSecrets()) && ok
        if (projectSecretsDirty) ok = (await saveProjectSecrets()) && ok
        if (ok) setCredentialsSavedDialogOpen(true)
      },
      onCancel: () => {
        resetGlobalEnv()
        resetProjectEnv()
        resetGlobalSecrets()
        resetProjectSecrets()
      },
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
          <TabsTrigger value="credentials">Credentials</TabsTrigger>
          {SHOW_JIRA_SETTINGS && <TabsTrigger value="integrations">Integrations</TabsTrigger>}
        </TabsList>

        {activeActions.dirty && <UnsavedChangesNotice />}
        {activeActions.error && <p className="text-sm text-destructive">{activeActions.error}</p>}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <TabsContent value="general">
          <GeneralTab
            defaultAgent={localDefaultAgent}
            setDefaultAgent={setLocalDefaultAgent}
            permissionMode={permissionMode}
            setPermissionMode={setPermissionMode}
            codexPermissionMode={codexPermissionMode}
            setCodexPermissionMode={setCodexPermissionMode}
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
            highlightPreset={highlightNetworkPreset}
            onHighlightPresetShown={() => setHighlightNetworkPreset(false)}
          />
        </TabsContent>

        <TabsContent value="credentials">
          <CredentialsTab
            globalVars={globalEnvVars}
            setGlobalVars={setGlobalEnvVars}
            projects={projects}
            selectedEnvProjectId={selectedEnvProjectId}
            setSelectedEnvProjectId={setSelectedEnvProjectId}
            projectVars={projectEnvVars}
            setProjectVars={setProjectEnvVars}
            loadingProjectVars={loadingProjectEnvVars}
            globalSecrets={globalSecrets}
            setGlobalSecrets={setGlobalSecrets}
            selectedSecretProjectId={selectedSecretProjectId}
            setSelectedSecretProjectId={setSelectedSecretProjectId}
            projectSecrets={projectSecrets}
            setProjectSecrets={setProjectSecrets}
            loadingProjectSecrets={loadingProjectSecrets}
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

      <EnvVarsSavedDialog open={credentialsSavedDialogOpen} onOpenChange={setCredentialsSavedDialogOpen} />
    </Tabs>
  )
}
