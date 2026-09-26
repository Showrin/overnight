import {
  Activity,
  Archive,
  KeyRound,
  Network,
  Plug,
  ScrollText,
  SlidersHorizontal,
  Terminal,
  type LucideIcon,
} from 'lucide-react'

export const SHOW_JIRA_SETTINGS = false
export const SHOW_PERFORMANCE_MONITOR = false

export type SettingsTab = 'general' | 'backups' | 'network' | 'credentials' | 'integrations'
export type SettingsSection = SettingsTab | 'performance-monitor' | 'daemon-logs' | 'command-logs'

export interface SectionItem {
  id: SettingsSection
  label: string
  icon: LucideIcon
}

export const SECTION_GROUPS: { label: string; items: SectionItem[] }[] = [
  {
    label: 'Settings',
    items: [
      { id: 'general', label: 'General', icon: SlidersHorizontal },
      { id: 'backups', label: 'Backups', icon: Archive },
      { id: 'network', label: 'Network', icon: Network },
      { id: 'credentials', label: 'Credentials', icon: KeyRound },
      ...(SHOW_JIRA_SETTINGS ? [{ id: 'integrations' as const, label: 'Integrations', icon: Plug }] : []),
    ],
  },
  {
    label: 'Developer Tools',
    items: [
      ...(SHOW_PERFORMANCE_MONITOR
        ? [{ id: 'performance-monitor' as const, label: 'Performance Monitor', icon: Activity }]
        : []),
      { id: 'daemon-logs', label: 'Daemon Logs', icon: ScrollText },
      { id: 'command-logs', label: 'Command Logs', icon: Terminal },
    ],
  },
]

const SETTINGS_TABS: readonly string[] = ['general', 'backups', 'network', 'credentials', 'integrations']

export function isSettingsTab(section: SettingsSection): section is SettingsTab {
  return SETTINGS_TABS.includes(section)
}

export function sectionLabel(section: SettingsSection): string {
  return SECTION_GROUPS.flatMap((g) => g.items).find((i) => i.id === section)?.label ?? 'Settings'
}
