import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { Project } from '@/components/projects/types'
import type { EnvVar } from '@/lib/envVars'
import type { Secret } from '@/lib/secrets'
import { EnvVarEditor } from '../EnvVarEditor'
import { SecretEditor } from '../SecretEditor'
import { SettingsRow, SettingsSection } from '../SettingsLayout'

interface CredentialsTabProps {
  globalVars: EnvVar[]
  setGlobalVars: (vars: EnvVar[]) => void
  projects: Project[]
  selectedEnvProjectId: string | null
  setSelectedEnvProjectId: (id: string) => void
  projectVars: EnvVar[]
  setProjectVars: (vars: EnvVar[]) => void
  loadingProjectVars: boolean
  globalSecrets: Secret[]
  setGlobalSecrets: (secrets: Secret[]) => void
  selectedSecretProjectId: string | null
  setSelectedSecretProjectId: (id: string) => void
  projectSecrets: Secret[]
  setProjectSecrets: (secrets: Secret[]) => void
  loadingProjectSecrets: boolean
}

export function CredentialsTab({
  globalVars,
  setGlobalVars,
  projects,
  selectedEnvProjectId,
  setSelectedEnvProjectId,
  projectVars,
  setProjectVars,
  loadingProjectVars,
  globalSecrets,
  setGlobalSecrets,
  selectedSecretProjectId,
  setSelectedSecretProjectId,
  projectSecrets,
  setProjectSecrets,
  loadingProjectSecrets,
}: CredentialsTabProps) {
  return (
    <>
      <SettingsSection title="Environment Variables" divider>
        <SettingsRow label="Global" description="Applied to every new sandbox, regardless of project." divider={false}>
          <EnvVarEditor vars={globalVars} onChange={setGlobalVars} />
        </SettingsRow>

        <SettingsRow
          label="Project"
          description="Applied to every new sandbox created for the selected project. Overrides global with the same name."
          divider={false}
        >
          <div className="flex flex-col gap-3">
            <Select value={selectedEnvProjectId ?? undefined} onValueChange={setSelectedEnvProjectId}>
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
            {selectedEnvProjectId &&
              (loadingProjectVars ? (
                <p className="text-xs text-muted-foreground">Loading…</p>
              ) : (
                <EnvVarEditor vars={projectVars} onChange={setProjectVars} />
              ))}
          </div>
        </SettingsRow>
      </SettingsSection>

      <SettingsSection title="Secrets">
        <SettingsRow label="Global" description="Applied to every new sandbox, regardless of project." divider={false}>
          <SecretEditor secrets={globalSecrets} onChange={setGlobalSecrets} />
        </SettingsRow>

        <SettingsRow
          label="Project"
          description="Applied to every new sandbox created for the selected project. Overrides global with the same env var name."
          divider={false}
        >
          <div className="flex flex-col gap-3">
            <Select value={selectedSecretProjectId ?? undefined} onValueChange={setSelectedSecretProjectId}>
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
            {selectedSecretProjectId &&
              (loadingProjectSecrets ? (
                <p className="text-xs text-muted-foreground">Loading…</p>
              ) : (
                <SecretEditor secrets={projectSecrets} onChange={setProjectSecrets} />
              ))}
          </div>
        </SettingsRow>
      </SettingsSection>
    </>
  )
}
