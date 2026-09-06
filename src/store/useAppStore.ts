import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { Project } from '@/components/projects/types'
import type { HostMetric, Sandbox } from '@/components/sandboxes/types'
import type { NetworkPolicySettings } from '@/lib/networkPolicy'

const SANDBOX_POLL_MS = 5000
// Orphan adoption shells out to `sbx ls` itself, so it runs far less often
// than the plain DB-backed sandbox poll above (~20s instead of every 5s).
const ORPHAN_ADOPTION_TICK_INTERVAL = 4
const HOST_STATS_HISTORY_SEED_MS = 24 * 60 * 60 * 1000
const HOST_STATS_HISTORY_MAX_POINTS = 500

export interface AppSettings {
  default_claude_permission_mode: string
  skill_folders: string[]
}

interface AppStore {
  projects: Project[]
  sandboxes: Sandbox[]
  settings: AppSettings | null
  platform: string | null
  defaultTerminalHost: string
  networkPolicyPreset: string | null
  hostStats: HostMetric | null
  hostStatsHistory: HostMetric[]

  loadProjects: () => Promise<void>
  loadSandboxes: () => Promise<void>
  loadSettings: () => Promise<void>
  saveSettings: (defaultClaudePermissionMode: string) => Promise<void>
  saveSkillFolders: (folders: string[]) => Promise<void>
  loadPlatform: () => Promise<void>
  loadDefaultTerminalHost: () => Promise<void>
  saveDefaultTerminalHost: (terminalHost: string) => Promise<void>
  loadNetworkPolicyPreset: () => Promise<void>
  loadHostStatsHistory: () => Promise<void>
  loadHostStats: () => Promise<void>
}

export const useAppStore = create<AppStore>((set, get) => ({
  projects: [],
  sandboxes: [],
  settings: null,
  platform: null,
  defaultTerminalHost: 'cmd',
  networkPolicyPreset: null,
  hostStats: null,
  hostStatsHistory: [],

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

  // Only the preset is cached here for SandboxCard's "effective preset"
  // display — the allow/deny rule lists are never mirrored in the store,
  // same "no local DB mirror" principle as the backend (see
  // commands.rs::get_network_policy_settings).
  async loadNetworkPolicyPreset() {
    const policy = await invoke<NetworkPolicySettings>('get_network_policy_settings')
    set({ networkPolicyPreset: policy.preset })
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
  useAppStore.getState().loadNetworkPolicyPreset()
  useAppStore.getState().loadHostStatsHistory()
  let tick = 0
  setInterval(() => {
    tick += 1
    useAppStore.getState().loadSandboxes()
    useAppStore.getState().loadHostStats()
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
