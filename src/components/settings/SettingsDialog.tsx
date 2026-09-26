import { CommandLogTab } from '@/components/developer/CommandLogTab'
import { TelemetryTab } from '@/components/developer/TelemetryTab'
import { PerformanceMonitorTab } from '@/components/sandboxes/PerformanceMonitorTab'
import { Dialog, DialogContent } from '@/components/ui/dialog'
import { useAppStore } from '@/store/useAppStore'
import { cn } from '@/lib/utils'
import { SECTION_GROUPS, SHOW_PERFORMANCE_MONITOR, isSettingsTab, sectionLabel, type SettingsSection } from './sections'
import { SettingsDialogHeader } from './SettingsLayout'
import { SettingsScreen } from './SettingsScreen'

function SettingsNav({ current, onSelect }: { current: SettingsSection; onSelect: (section: SettingsSection) => void }) {
  return (
    <nav className="flex w-52 shrink-0 flex-col gap-6 overflow-auto border-r border-sidebar-border bg-sidebar p-2 pt-5">
      {SECTION_GROUPS.map((group) => (
        <div key={group.label} className="flex flex-col gap-1">
          <span className="px-2.5 text-xs text-muted-foreground">{group.label}</span>
          {group.items.map(({ id, label, icon: Icon }) => {
            const isActive = current === id
            return (
              <button
                key={id}
                type="button"
                onClick={() => onSelect(id)}
                className={cn(
                  'relative flex items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm transition-colors',
                  isActive
                    ? 'bg-sidebar-accent text-sidebar-accent-foreground shadow-sm'
                    : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground'
                )}
              >
                {isActive && <span className="absolute top-1 bottom-1 left-0 w-0.5 rounded-full bg-primary" />}
                <Icon className="size-4 shrink-0" strokeWidth={1.5} />
                {label}
              </button>
            )
          })}
        </div>
      ))}
    </nav>
  )
}

export function SettingsDialog() {
  const section = useAppStore((s) => s.settingsDialogSection)
  const openSettings = useAppStore((s) => s.openSettings)
  const closeSettings = useAppStore((s) => s.closeSettings)

  return (
    <Dialog open={section !== null} onOpenChange={(open) => !open && closeSettings()}>
      <DialogContent
        title="Settings"
        className="flex h-[85vh] w-[calc(100vw-4rem)] max-w-6xl overflow-hidden rounded-xl border border-border bg-background p-0"
      >
        {section && (
          <>
            <SettingsNav current={section} onSelect={openSettings} />
            <div className="@container flex min-w-0 flex-1 flex-col overflow-hidden">
              {!isSettingsTab(section) && <SettingsDialogHeader title={sectionLabel(section)} />}
              <div className={cn('flex min-h-0 flex-1 flex-col', !isSettingsTab(section) && 'hidden')}>
                <SettingsScreen section={isSettingsTab(section) ? section : 'general'} />
              </div>
              {SHOW_PERFORMANCE_MONITOR && section === 'performance-monitor' && (
                <div className="min-h-0 flex-1 overflow-auto px-6 pt-4 pb-6">
                  <PerformanceMonitorTab />
                </div>
              )}
              {section === 'daemon-logs' && (
                <div className="flex min-h-0 flex-1 flex-col px-6 pt-4 pb-6">
                  <TelemetryTab />
                </div>
              )}
              {section === 'command-logs' && (
                <div className="flex min-h-0 flex-1 flex-col px-6 pt-4 pb-6">
                  <CommandLogTab />
                </div>
              )}
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
