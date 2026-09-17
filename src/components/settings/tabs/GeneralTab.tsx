import { useState } from 'react'
import { Search } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { PERMISSION_MODES } from '@/lib/permissionModes'
import { TERMINAL_HOSTS, TERMINAL_HOST_LABELS } from '@/lib/terminalHost'
import { SettingsRow } from '../SettingsLayout'

// Skills are named after their folder, so the last path segment is the
// skill name. Splits on both separators since paths come from the host OS.
function skillName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean)
  return segments[segments.length - 1] ?? path
}

interface GeneralTabProps {
  permissionMode: string
  setPermissionMode: (mode: string) => void
  terminalHost: string
  setTerminalHost: (host: string) => void
  skillFolders: string[]
  onAddSkillFolders: () => void
  onRemoveSkillFolder: (path: string) => void
}

export function GeneralTab({
  permissionMode,
  setPermissionMode,
  terminalHost,
  setTerminalHost,
  skillFolders,
  onAddSkillFolders,
  onRemoveSkillFolder,
}: GeneralTabProps) {
  const [search, setSearch] = useState('')

  const query = search.trim().toLowerCase()
  const matches = query ? skillFolders.filter((path) => skillName(path).toLowerCase().includes(query)) : skillFolders

  return (
    <>
      <SettingsRow
        label="Default Claude permission mode"
        description="The permission mode new Claude sessions start in."
      >
        <Select value={permissionMode} onValueChange={setPermissionMode}>
          <SelectTrigger className="max-w-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PERMISSION_MODES.map((mode) => (
              <SelectItem key={mode} value={mode}>
                {mode}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingsRow>

      <SettingsRow
        label="Default terminal host"
        description="Which console app opens the sandbox terminal on Windows. Has no effect on macOS/Linux."
      >
        <Select value={terminalHost} onValueChange={setTerminalHost}>
          <SelectTrigger className="max-w-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TERMINAL_HOSTS.map((host) => (
              <SelectItem key={host} value={host}>
                {TERMINAL_HOST_LABELS[host]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingsRow>

      <SettingsRow label="Skill folders" description="Folders scanned for Claude skills.">
        <div className="flex gap-2">
          {skillFolders.length > 0 && (
            <div className="relative min-w-0 flex-1">
              <Search
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                strokeWidth={1.5}
              />
              <Input
                className="pl-8"
                placeholder="Search skills"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          )}
          <Button type="button" variant="outline" onClick={onAddSkillFolders}>
            Add folder(s)
          </Button>
        </div>
        {skillFolders.length > 0 &&
          (matches.length === 0 ? (
            <p className="text-xs text-muted-foreground">No skills match "{search}".</p>
          ) : (
            <ul className="flex flex-col gap-1">
              {matches.map((path) => (
                <li
                  key={path}
                  className="flex items-center justify-between gap-2 rounded-lg border border-border px-2.5 py-1.5 text-sm"
                >
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate">{skillName(path)}</span>
                    <span className="truncate text-xs text-muted-foreground">{path}</span>
                  </span>
                  <button
                    type="button"
                    onClick={() => onRemoveSkillFolder(path)}
                    className="shrink-0 text-muted-foreground hover:text-destructive"
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          ))}
      </SettingsRow>
    </>
  )
}
