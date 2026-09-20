import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { Project } from '@/components/projects/types'
import type { BranchSyncOutcome, HostMetric, Sandbox } from '@/components/sandboxes/types'
import type { ActiveBackup, ActiveOperation } from '@/components/backups/types'
import type { NetworkPolicySettings } from '@/lib/networkPolicy'

const SANDBOX_POLL_MS = 5000
// Orphan adoption shells out to `sbx ls` itself, so it runs far less often
// than the plain DB-backed sandbox poll above (~20s instead of every 5s).
const ORPHAN_ADOPTION_TICK_INTERVAL = 4
const HOST_STATS_HISTORY_SEED_MS = 24 * 60 * 60 * 1000
const HOST_STATS_HISTORY_MAX_POINTS = 150

export interface AppSettings {
  default_claude_permission_mode: string
  skill_folders: string[]
}

// State for the bottom-right Git Sync popout — set once per sync run,
// cleared on dismiss or by GitSyncToast's own 45s auto-dismiss timer.
export interface GitSyncToastState {
  sandboxId: string
  label: string
  outcomes: BranchSyncOutcome[]
}

interface AppStore {
  projects: Project[]
  sandboxes: Sandbox[]
  settings: AppSettings | null
  platform: string | null
  defaultTerminalHost: string
  layoutExpanded: boolean
  defaultAgent: string
  networkPolicyPreset: string | null
  highlightNetworkPreset: boolean
  hostStats: HostMetric | null
  hostStatsHistory: HostMetric[]
  backupIntervalMinutes: number
  autoBackupEnabled: boolean
  activeBackups: ActiveBackup[]
  activeOperations: ActiveOperation[]
  sidebarWidth: number
  gitSyncToast: GitSyncToastState | null

  loadProjects: () => Promise<void>
  loadSandboxes: () => Promise<void>
  loadSettings: () => Promise<void>
  saveSettings: (defaultClaudePermissionMode: string) => Promise<void>
  saveSkillFolders: (folders: string[]) => Promise<void>
  loadPlatform: () => Promise<void>
  loadDefaultTerminalHost: () => Promise<void>
  saveDefaultTerminalHost: (terminalHost: string) => Promise<void>
  loadLayoutExpanded: () => Promise<void>
  saveLayoutExpanded: (expanded: boolean) => Promise<void>
  loadDefaultAgent: () => Promise<void>
  saveDefaultAgent: (agent: string) => Promise<void>
  loadNetworkPolicyPreset: () => Promise<void>
  setHighlightNetworkPreset: (value: boolean) => void
  loadHostStatsHistory: () => Promise<void>
  loadHostStats: () => Promise<void>
  loadBackupIntervalMinutes: () => Promise<void>
  saveBackupIntervalMinutes: (minutes: number) => Promise<void>
  loadAutoBackupEnabled: () => Promise<void>
  saveAutoBackupEnabled: (enabled: boolean) => Promise<void>
  loadActiveBackups: () => Promise<void>
  loadActiveOperations: () => Promise<void>
  loadSidebarWidth: () => Promise<void>
  setSidebarWidth: (width: number) => void
  saveSidebarWidth: (width: number) => Promise<void>
  showGitSyncToast: (toast: GitSyncToastState) => void
  dismissGitSyncToast: () => void
}

export const useAppStore = create<AppStore>((set, get) => ({
  projects: [],
  sandboxes: [],
  settings: null,
  platform: null,
  defaultTerminalHost: 'cmd',
  layoutExpanded: false,
  defaultAgent: 'claude',
  networkPolicyPreset: null,
  highlightNetworkPreset: false,
  hostStats: null,
  hostStatsHistory: [],
  backupIntervalMinutes: 15,
  autoBackupEnabled: true,
  activeBackups: [],
  activeOperations: [],
  sidebarWidth: 208,
  gitSyncToast: null,

  async loadProjects() {
    const projects = await invoke<Project[]>('list_projects')
    set({ projects })
  },

  async loadSandboxes() {
    const sandboxes = await invoke<Sandbox[]>('list_sandboxes')
    set({ sandboxes })
  },

  async loadSettings() {
    const settings = await invoke<AppSettings>('get_settings')
    set({ settings })
  },

  async saveSettings(defaultClaudePermissionMode) {
    await invoke('save_settings', { defaultClaudePermissionMode })
    set((state) => ({
      settings: {
        default_claude_permission_mode: defaultClaudePermissionMode,
        skill_folders: state.settings?.skill_folders ?? [],
      },
    }))
  },

  async saveSkillFolders(folders) {
    await invoke('save_skill_folders', { folders })
    set((state) => ({
      settings: {
        default_claude_permission_mode: state.settings?.default_claude_permission_mode ?? 'default',
        skill_folders: folders,
      },
    }))
  },

  async loadPlatform() {
    const platform = await invoke<string>('get_platform')
    set({ platform })
  },

  async loadDefaultTerminalHost() {
    const terminalHost = await invoke<string>('get_default_terminal_host')
    set({ defaultTerminalHost: terminalHost })
  },

  async saveDefaultTerminalHost(terminalHost) {
    await invoke('save_default_terminal_host', { terminalHost })
    set({ defaultTerminalHost: terminalHost })
  },

  async loadLayoutExpanded() {
    const expanded = await invoke<boolean>('get_layout_expanded')
    set({ layoutExpanded: expanded })
  },

  async saveLayoutExpanded(expanded) {
    await invoke('save_layout_expanded', { expanded })
    set({ layoutExpanded: expanded })
  },

  async loadDefaultAgent() {
    const agent = await invoke<string>('get_default_agent')
    set({ defaultAgent: agent })
  },

  async saveDefaultAgent(agent) {
    await invoke('save_default_agent', { agent })
    set({ defaultAgent: agent })
  },

  // Only the preset is cached here for SandboxCard's "effective preset"
  // display — the allow/deny rule lists are never mirrored in the store,
  // same "no local DB mirror" principle as the backend (see
  // commands.rs::get_network_policy_settings).
  async loadNetworkPolicyPreset() {
    const policy = await invoke<NetworkPolicySettings>('get_network_policy_settings')
    set({ networkPolicyPreset: policy.preset })
  },

  setHighlightNetworkPreset(value) {
    set({ highlightNetworkPreset: value })
  },

  async loadHostStatsHistory() {
    const history = await invoke<HostMetric[]>('get_host_stats_history', {
      sinceMs: Date.now() - HOST_STATS_HISTORY_SEED_MS,
    })
    set({ hostStatsHistory: history.slice(-HOST_STATS_HISTORY_MAX_POINTS) })
  },

  async loadHostStats() {
    const stats = await invoke<HostMetric>('get_host_stats')
    set({ hostStats: stats, hostStatsHistory: [...get().hostStatsHistory, stats].slice(-HOST_STATS_HISTORY_MAX_POINTS) })
  },

  async loadBackupIntervalMinutes() {
    const minutes = await invoke<number>('get_backup_interval_minutes')
    set({ backupIntervalMinutes: minutes })
  },

  async saveBackupIntervalMinutes(minutes) {
    await invoke('save_backup_interval_minutes', { minutes })
    set({ backupIntervalMinutes: minutes })
  },

  async loadAutoBackupEnabled() {
    const enabled = await invoke<boolean>('get_auto_backup_enabled')
    set({ autoBackupEnabled: enabled })
  },

  async saveAutoBackupEnabled(enabled) {
    await invoke('save_auto_backup_enabled', { enabled })
    set({ autoBackupEnabled: enabled })
  },

  async loadActiveBackups() {
    const activeBackups = await invoke<ActiveBackup[]>('list_active_backups')
    set({ activeBackups })
  },

  async loadActiveOperations() {
    const activeOperations = await invoke<ActiveOperation[]>('list_active_operations')
    set({ activeOperations })
  },

  async loadSidebarWidth() {
    const width = await invoke<number>('get_sidebar_width')
    set({ sidebarWidth: width })
  },

  setSidebarWidth(width) {
    set({ sidebarWidth: width })
  },

  async saveSidebarWidth(width) {
    await invoke('save_sidebar_width', { width })
    set({ sidebarWidth: width })
  },

  showGitSyncToast(toast) {
    set({ gitSyncToast: toast })
  },

  dismissGitSyncToast() {
    set({ gitSyncToast: null })
  },
}))

let initialized = false

/**
 * Loads projects/sandboxes/settings once and keeps sandbox status fresh
 * across the whole app (sidebar quick-actions, the Sandboxes screen, etc.)
 * with a single shared poll instead of every consumer running its own.
 * Call once from the app root; safe to call multiple times (e.g. React
 * StrictMode's double-effect in dev).
 */
export function initAppStore() {
  if (initialized) return
  initialized = true
  useAppStore.getState().loadProjects()
  useAppStore.getState().loadSandboxes()
  useAppStore.getState().loadSettings()
  useAppStore.getState().loadPlatform()
  useAppStore.getState().loadDefaultTerminalHost()
  useAppStore.getState().loadLayoutExpanded()
  useAppStore.getState().loadDefaultAgent()
  useAppStore.getState().loadNetworkPolicyPreset()
  useAppStore.getState().loadHostStatsHistory()
  useAppStore.getState().loadBackupIntervalMinutes()
  useAppStore.getState().loadAutoBackupEnabled()
  useAppStore.getState().loadActiveBackups()
  useAppStore.getState().loadActiveOperations()
  useAppStore.getState().loadSidebarWidth()
  let tick = 0
  setInterval(() => {
    tick += 1
    useAppStore.getState().loadSandboxes()
    useAppStore.getState().loadHostStats()
    useAppStore.getState().loadActiveBackups()
    useAppStore.getState().loadActiveOperations()
    if (tick % ORPHAN_ADOPTION_TICK_INTERVAL === 0) {
      adoptOrphanSandboxes()
    }
  }, SANDBOX_POLL_MS)
}

/**
 * Finds sandboxes `sbx ls` knows about with no matching DB row and creates
 * entries for them, then refreshes both sandboxes (the new rows) and
 * projects (a first adoption may have just lazily created the
 * "Unassigned" project). Best-effort — a failure here (e.g. `sbx` not on
 * PATH) shouldn't break the rest of the poll loop.
 */
async function adoptOrphanSandboxes() {
  try {
    await invoke('adopt_orphan_sandboxes')
    await Promise.all([useAppStore.getState().loadSandboxes(), useAppStore.getState().loadProjects()])
  } catch (e) {
    console.error('adoptOrphanSandboxes failed:', e)
  }
}
