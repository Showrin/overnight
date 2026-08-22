import { Box, FolderGit2, LayoutDashboard, Settings } from 'lucide-react'

export type Screen = 'dashboard' | 'projects' | 'sandboxes' | 'settings'

const items: { screen: Screen; label: string; icon: typeof LayoutDashboard }[] = [
  { screen: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { screen: 'projects', label: 'Projects', icon: FolderGit2 },
  { screen: 'sandboxes', label: 'Sandboxes', icon: Box },
  { screen: 'settings', label: 'Settings', icon: Settings },
]

export function Sidebar({
  current,
  onNavigate,
}: {
  current: Screen
  onNavigate: (screen: Screen) => void
}) {
  return (
    <nav className="flex w-48 shrink-0 flex-col gap-1 border-r border-sidebar-border bg-sidebar p-2">
      {items.map(({ screen, label, icon: Icon }) => (
        <button
          key={screen}
          type="button"
          onClick={() => onNavigate(screen)}
          className={`flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground ${
            current === screen ? 'bg-sidebar-accent text-sidebar-accent-foreground' : ''
          }`}
        >
          <Icon className="size-4" strokeWidth={1.5} />
          {label}
        </button>
      ))}
    </nav>
  )
}
