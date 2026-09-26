import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { EnvVar } from '@/lib/envVars'
import type { NetworkPolicySettings, NetworkRuleDecision } from '@/lib/networkPolicy'
import type { Secret } from '@/lib/secrets'
import { useAppStore } from '@/store/useAppStore'
import type { JiraConfig } from './JiraConfigForm'
import { EnvVarsSavedDialog } from './EnvVarsSavedDialog'
import { SettingsActions, SettingsDialogHeader, type TabActions } from './SettingsLayout'
import { SHOW_JIRA_SETTINGS, type SettingsTab, sectionLabel } from './sections'
import { BackupsTab } from './tabs/BackupsTab'
import { CredentialsTab } from './tabs/CredentialsTab'
import { GeneralTab } from './tabs/GeneralTab'
import { IntegrationsTab } from './tabs/IntegrationsTab'
import { NetworkTab } from './tabs/NetworkTab'

function sameFolders(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((path, i) => path === b[i])
}

export function SettingsScreen({ section }: { section: SettingsTab }) {
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

  const savedPermissionMode = settings?.default_permission_mode ?? 'never'
  const savedSkillFolders = settings?.skill_folders ?? []

  const [permissionMode, setPermissionMode] = useState<string>('never')
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
      setPermissionMode(settings.default_permission_mode)
      setSkillFolders(settings.skill_folders)
    }
  }, [settings])

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
    localDefaultAgent !== defaultAgent ||
    terminalHost !== defaultTerminalHost ||
    !sameFolders(skillFolders, savedSkillFolders)

  async function saveGeneral() {
    setSavingGeneral(true)
    setGeneralError(null)
    try {
      // permissionMode is only ever meaningful together with the agent it
      // was chosen for (see permissionModes.ts) — pass localDefaultAgent
      // explicitly rather than relying on saveDefaultAgent landing first.
      if (permissionMode !== savedPermissionMode || localDefaultAgent !== defaultAgent) {
        await saveSettings(localDefaultAgent, permissionMode)
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

  const activeActions = tabActions[section]

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <SettingsDialogHeader title={sectionLabel(section)} dirty={activeActions.dirty} />
      {activeActions.error && <p className="shrink-0 px-6 pt-4 text-sm text-destructive">{activeActions.error}</p>}

      <div className="min-h-0 flex-1 overflow-auto px-6 pb-6">
        {section === 'general' && (
          <GeneralTab
            defaultAgent={localDefaultAgent}
            setDefaultAgent={setLocalDefaultAgent}
            permissionMode={permissionMode}
            setPermissionMode={setPermissionMode}
            terminalHost={terminalHost}
            setTerminalHost={setTerminalHost}
            skillFolders={skillFolders}
            onAddSkillFolders={addSkillFolders}
            onRemoveSkillFolder={removeSkillFolder}
          />
        )}

        {section === 'backups' && (
          <BackupsTab
            autoBackup={autoBackup}
            setAutoBackup={setAutoBackup}
            backupInterval={backupInterval}
            setBackupInterval={setBackupInterval}
          />
        )}

        {section === 'network' && (
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
        )}

        {section === 'credentials' && (
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
        )}

        {SHOW_JIRA_SETTINGS && section === 'integrations' && (
          <IntegrationsTab
            jiraConfig={jiraConfig}
            editingJira={editingJira}
            setEditingJira={setEditingJira}
            onJiraSaved={loadJiraConfig}
          />
        )}
      </div>

      {section !== 'integrations' && (
        <div className="flex shrink-0 justify-end border-t border-border px-6 py-3">
          <SettingsActions {...activeActions} />
        </div>
      )}

      <EnvVarsSavedDialog open={credentialsSavedDialogOpen} onOpenChange={setCredentialsSavedDialogOpen} />
    </div>
  )
}
