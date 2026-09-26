import { useCallback, useRef } from 'react'
import { Archive, Box, FolderGit2, Settings } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SidebarSandboxList } from '@/components/sandboxes/SidebarSandboxList'
import { useAppStore } from '@/store/useAppStore'
import type { Route, Screen } from '@/lib/router'

const MIN_WIDTH = 208
const MAX_WIDTH = 360

type NavItem = { screen: Screen; label: string; icon: typeof Settings }

const groups: { label: string; items: NavItem[] }[] = [
  {
    label: 'Workspace',
    items: [
      { screen: 'projects', label: 'Projects', icon: FolderGit2 },
      { screen: 'sandboxes', label: 'Sandboxes', icon: Box },
      { screen: 'backups', label: 'Backups', icon: Archive },
    ],
  },
]

export function Sidebar({
  current,
  onNavigate,
  collapsed,
}: {
  current: Screen
  onNavigate: (route: Route) => void
  collapsed: boolean
}) {
  const sidebarWidth = useAppStore((s) => s.sidebarWidth)
  const setSidebarWidth = useAppStore((s) => s.setSidebarWidth)
  const saveSidebarWidth = useAppStore((s) => s.saveSidebarWidth)
  const openSettings = useAppStore((s) => s.openSettings)
  const resizing = useRef(false)

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      resizing.current = true
      const startX = e.clientX
      const startWidth = sidebarWidth
      let finalWidth = startWidth

      function onMouseMove(ev: MouseEvent) {
        if (!resizing.current) return
        finalWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startWidth + (ev.clientX - startX)))
        setSidebarWidth(finalWidth)
      }
      function onMouseUp() {
        resizing.current = false
        window.removeEventListener('mousemove', onMouseMove)
        window.removeEventListener('mouseup', onMouseUp)
        saveSidebarWidth(finalWidth)
      }
      window.addEventListener('mousemove', onMouseMove)
      window.addEventListener('mouseup', onMouseUp)
    },
    [sidebarWidth, setSidebarWidth, saveSidebarWidth]
  )

  if (collapsed) return null

  return (
    <nav
      className="relative flex shrink-0 flex-col border-r border-sidebar-border bg-sidebar"
      style={{ width: sidebarWidth }}
    >
      <div className="flex flex-1 flex-col gap-8 overflow-auto p-2 pt-[20px]">
        {groups.map((group) => (
          <div key={group.label} className="flex flex-col gap-1">
            <span className="px-2.5 text-xs font-normal text-muted-foreground">{group.label}</span>
            {group.items.map(({ screen, label, icon: Icon }) => {
              const isActive = current === screen
              return (
                <button
                  key={screen}
                  type="button"
                  onClick={() => onNavigate({ screen })}
                  className={`relative flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm transition-colors ${
                    isActive
                      ? 'bg-sidebar-accent text-sidebar-accent-foreground shadow-sm'
                      : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground'
                  }`}
                >
                  {isActive && (
                    <span className="absolute top-1 bottom-1 left-0 w-0.5 rounded-full bg-primary" />
                  )}
                  <Icon className="size-4 shrink-0" strokeWidth={1.5} />
                  {label}
                </button>
              )
            })}
          </div>
        ))}
      </div>
      <SidebarSandboxList onNavigate={onNavigate} />
      <div className="flex items-center justify-between border-t border-sidebar-border py-1.5 pr-1.5 pl-3 text-xs text-muted-foreground">
        <span>Overnight v0.0.0</span>
        <Button variant="ghost" size="icon-sm" aria-label="Settings" onClick={() => openSettings()}>
          <Settings className="size-4" strokeWidth={1.5} />
        </Button>
      </div>
      <div
        onMouseDown={handleMouseDown}
        className="absolute right-0 top-0 h-full w-1 cursor-col-resize select-none hover:bg-accent active:bg-accent"
      />
    </nav>
  )
}
