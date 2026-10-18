import {
  Activity,
  Archive,
  Boxes,
  Container,
  Download,
  Globe,
  KeyRound,
  Network,
  Plug,
  ScrollText,
  SlidersHorizontal,
  Terminal,
  Webhook,
  type LucideIcon,
} from 'lucide-react'

export const SHOW_JIRA_SETTINGS = false
export const SHOW_PERFORMANCE_MONITOR = false

export type SettingsTab = 'general' | 'backups' | 'network' | 'credentials' | 'kits' | 'compose' | 'browser' | 'integrations'
export type SettingsSection = SettingsTab | 'agent-config' | 'performance-monitor' | 'daemon-logs' | 'command-logs' | 'updates'

export interface SectionItem {
  id: SettingsSection
  label: string
  icon: LucideIcon
  experimental?: boolean
}

export const SECTION_GROUPS: { label: string; items: SectionItem[] }[] = [
  {
    label: 'Settings',
    items: [
      { id: 'general', label: 'General', icon: SlidersHorizontal },
      { id: 'backups', label: 'Backups', icon: Archive },
      { id: 'network', label: 'Network', icon: Network },
      { id: 'credentials', label: 'Credentials', icon: KeyRound },
      { id: 'kits', label: 'Kits', icon: Boxes, experimental: true },
      { id: 'compose', label: 'Compose Overrides', icon: Container, experimental: true },
      { id: 'browser', label: 'Browser Testing', icon: Globe, experimental: true },
      { id: 'agent-config', label: 'Hooks & Styles', icon: Webhook, experimental: true },
      ...(SHOW_JIRA_SETTINGS ? [{ id: 'integrations' as const, label: 'Integrations', icon: Plug }] : []),
      { id: 'updates', label: 'Updates', icon: Download },
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

const SETTINGS_TABS: readonly string[] = ['general', 'backups', 'network', 'credentials', 'kits', 'compose', 'browser', 'integrations']

export function isSettingsTab(section: SettingsSection): section is SettingsTab {
  return SETTINGS_TABS.includes(section)
}

export function sectionLabel(section: SettingsSection): string {
  return SECTION_GROUPS.flatMap((g) => g.items).find((i) => i.id === section)?.label ?? 'Settings'
}

export function isExperimental(section: SettingsSection): boolean {
  return SECTION_GROUPS.flatMap((g) => g.items).find((i) => i.id === section)?.experimental ?? false
}
