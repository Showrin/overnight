import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { Project } from '@/components/projects/types'
import type { EnvVar } from '@/lib/envVars'
import { EnvVarEditor } from '../EnvVarEditor'
import { SettingsRow } from '../SettingsLayout'

interface CredentialsTabProps {
  globalVars: EnvVar[]
  setGlobalVars: (vars: EnvVar[]) => void
  projects: Project[]
  selectedProjectId: string | null
  setSelectedProjectId: (id: string) => void
  projectVars: EnvVar[]
  setProjectVars: (vars: EnvVar[]) => void
  loadingProjectVars: boolean
}

export function CredentialsTab({
  globalVars,
  setGlobalVars,
  projects,
  selectedProjectId,
  setSelectedProjectId,
  projectVars,
  setProjectVars,
  loadingProjectVars,
}: CredentialsTabProps) {
  return (
    <>
      <SettingsRow label="Global credentials" description="Applied to every new sandbox, regardless of project.">
        <EnvVarEditor vars={globalVars} onChange={setGlobalVars} />
      </SettingsRow>

      <SettingsRow
        label="Project credentials"
        description="Applied to every new sandbox created for the selected project. Overrides global credentials with the same name."
      >
        <div className="flex flex-col gap-3">
          <Select value={selectedProjectId ?? undefined} onValueChange={setSelectedProjectId}>
            <SelectTrigger className="max-w-sm">
              <SelectValue placeholder="Select a project" />
            </SelectTrigger>
            <SelectContent>
              {projects.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {selectedProjectId &&
            (loadingProjectVars ? (
              <p className="text-xs text-muted-foreground">Loading…</p>
            ) : (
              <EnvVarEditor vars={projectVars} onChange={setProjectVars} />
            ))}
        </div>
      </SettingsRow>
    </>
  )
}
