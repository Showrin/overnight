import { Box, FolderGit2, LayoutDashboard, Settings } from 'lucide-react'
import logoIcon from '@/assets/logo-icon.svg'

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
}: {
  current: Screen
  onNavigate: (screen: Screen) => void
}) {
  return (
    <nav className="flex w-52 shrink-0 flex-col border-r border-sidebar-border bg-sidebar">
      <div className="flex items-center gap-2 border-b border-sidebar-border px-3 py-3">
        <img src={logoIcon} alt="" className="h-6 w-auto rounded-[5px]" />
        <span className="text-sm font-medium text-sidebar-foreground">Overnight</span>
      </div>
      <div className="flex flex-1 flex-col gap-4 overflow-auto p-2">
        {groups.map((group) => (
          <div key={group.label} className="flex flex-col gap-1">
            <span className="px-2.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">
              {group.label}
            </span>
            {group.items.map(({ screen, label, icon: Icon }) => {
              const isActive = current === screen
              return (
                <button
                  key={screen}
                  type="button"
                  onClick={() => onNavigate(screen)}
                  className={`relative flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm transition-colors ${
                    isActive
                      ? 'bg-sidebar-accent text-sidebar-accent-foreground shadow-sm'
                      : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground'
                  }`}
                >
                  {isActive && (
                    <span className="absolute top-1 bottom-1 left-0 w-0.5 rounded-full bg-primary" />
                  )}
                  <Icon className="size-4" strokeWidth={1.5} />
                  {label}
                </button>
              )
            })}
          </div>
        ))}
      </div>
      <div className="border-t border-sidebar-border px-3 py-2.5 text-xs text-muted-foreground">
        Overnight v0.0.0
      </div>
    </nav>
  )
}
