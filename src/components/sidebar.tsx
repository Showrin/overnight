import { Box, FolderGit2, LayoutDashboard, Settings } from 'lucide-react'
import { SidebarSandboxList } from '@/components/sandboxes/SidebarSandboxList'

export type Screen = 'dashboard' | 'projects' | 'sandboxes' | 'settings'

type NavItem = { screen: Screen; label: string; icon: typeof LayoutDashboard }

const groups: { label: string; items: NavItem[] }[] = [
  {
    label: 'Overview',
    items: [{ screen: 'dashboard', label: 'Dashboard', icon: LayoutDashboard }],
  },
  {
    label: 'Workspace',
    items: [
      { screen: 'projects', label: 'Projects', icon: FolderGit2 },
      { screen: 'sandboxes', label: 'Sandboxes', icon: Box },
    ],
  },
  {
    label: 'System',
    items: [{ screen: 'settings', label: 'Settings', icon: Settings }],
  },
]

export function Sidebar({
  current,
  onNavigate,
  collapsed,
}: {
  current: Screen
  onNavigate: (screen: Screen) => void
  collapsed: boolean
}) {
  return (
    <nav
      className={`flex shrink-0 flex-col border-r border-sidebar-border bg-sidebar transition-[width] ${
        collapsed ? 'w-14' : 'w-52'
      }`}
    >
      <div className="flex flex-1 flex-col gap-4 overflow-auto p-2">
        {groups.map((group) => (
          <div key={group.label} className="flex flex-col gap-1">
            {!collapsed && (
              <span className="px-2.5 text-xs font-normal text-muted-foreground">{group.label}</span>
            )}
            {group.items.map(({ screen, label, icon: Icon }) => {
              const isActive = current === screen
              return (
                <button
                  key={screen}
                  type="button"
                  title={collapsed ? label : undefined}
                  onClick={() => onNavigate(screen)}
                  className={`relative flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm transition-colors ${
                    collapsed ? 'justify-center' : ''
                  } ${
                    isActive
                      ? 'bg-sidebar-accent text-sidebar-accent-foreground shadow-sm'
                      : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground'
                  }`}
                >
                  {isActive && (
                    <span className="absolute top-1 bottom-1 left-0 w-0.5 rounded-full bg-primary" />
                  )}
                  <Icon className="size-4 shrink-0" strokeWidth={1.5} />
                  {!collapsed && label}
                </button>
              )
            })}
          </div>
        ))}
      </div>
      {!collapsed && <SidebarSandboxList onNavigate={onNavigate} />}
      {!collapsed && (
        <div className="border-t border-sidebar-border px-3 py-2.5 text-xs text-muted-foreground">
          Overnight v0.0.0
        </div>
      )}
    </nav>
  )
}
