import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { Project } from '@/components/projects/types'
import type { HostMetric, Sandbox } from '@/components/sandboxes/types'

const SANDBOX_POLL_MS = 5000
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
  hostStats: HostMetric | null
  hostStatsHistory: HostMetric[]

  loadProjects: () => Promise<void>
  loadSandboxes: () => Promise<void>
  loadSettings: () => Promise<void>
  saveSettings: (defaultClaudePermissionMode: string) => Promise<void>
  saveSkillFolders: (folders: string[]) => Promise<void>
  loadHostStatsHistory: () => Promise<void>
  loadHostStats: () => Promise<void>
}

export const useAppStore = create<AppStore>((set, get) => ({
  projects: [],
  sandboxes: [],
  settings: null,
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
  useAppStore.getState().loadHostStatsHistory()
  setInterval(() => {
    useAppStore.getState().loadSandboxes()
    useAppStore.getState().loadHostStats()
  }, SANDBOX_POLL_MS)
}
