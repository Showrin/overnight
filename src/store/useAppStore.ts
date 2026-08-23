import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { Project } from '@/components/projects/types'
import type { Sandbox } from '@/components/sandboxes/types'

const SANDBOX_POLL_MS = 5000

export interface AppSettings {
  default_claude_permission_mode: string
}

interface AppStore {
  projects: Project[]
  sandboxes: Sandbox[]
  settings: AppSettings | null

  loadProjects: () => Promise<void>
  loadSandboxes: () => Promise<void>
  loadSettings: () => Promise<void>
  saveSettings: (defaultClaudePermissionMode: string) => Promise<void>
}

export const useAppStore = create<AppStore>((set) => ({
  projects: [],
  sandboxes: [],
  settings: null,

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
    set({ settings: { default_claude_permission_mode: defaultClaudePermissionMode } })
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
  setInterval(() => useAppStore.getState().loadSandboxes(), SANDBOX_POLL_MS)
}
