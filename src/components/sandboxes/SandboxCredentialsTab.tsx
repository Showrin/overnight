import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Info } from 'lucide-react'
import { EnvVarEditor } from '@/components/settings/EnvVarEditor'
import { EnvVarsSavedDialog } from '@/components/settings/EnvVarsSavedDialog'
import { SecretEditor } from '@/components/settings/SecretEditor'
import { SettingsActions, SettingsSection } from '@/components/settings/SettingsLayout'
import { Button } from '@/components/ui/button'
import type { EnvVar } from '@/lib/envVars'
import type { Secret } from '@/lib/secrets'
import { useAppStore } from '@/store/useAppStore'
import type { Sandbox } from './types'

function sameEnvVars(a: EnvVar[], b: EnvVar[]): boolean {
  return a.length === b.length && a.every((v, i) => v.key === b[i].key && v.value === b[i].value)
}

function sameSecrets(a: Secret[], b: Secret[]): boolean {
  return a.length === b.length && a.every((v, i) => JSON.stringify(v) === JSON.stringify(b[i]))
}

export function SandboxCredentialsTab({ sandbox }: { sandbox: Sandbox }) {
  const openSettings = useAppStore((s) => s.openSettings)
  const [vars, setVars] = useState<EnvVar[]>([])
  const [saved, setSaved] = useState<EnvVar[]>([])
  const [loading, setLoading] = useState(true)

  const [secrets, setSecrets] = useState<Secret[]>([])
  const [savedSecrets, setSavedSecrets] = useState<Secret[]>([])
  const [loadingSecrets, setLoadingSecrets] = useState(true)

  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [globalVars, setGlobalVars] = useState<EnvVar[]>([])
  const [loadingGlobal, setLoadingGlobal] = useState(true)

  const [projectVars, setProjectVars] = useState<EnvVar[]>([])
  const [loadingProject, setLoadingProject] = useState(true)

  const [globalSecrets, setGlobalSecrets] = useState<Secret[]>([])
  const [loadingGlobalSecrets, setLoadingGlobalSecrets] = useState(true)

  const [projectSecrets, setProjectSecrets] = useState<Secret[]>([])
  const [loadingProjectSecrets, setLoadingProjectSecrets] = useState(true)

  const [savedDialogOpen, setSavedDialogOpen] = useState(false)

  useEffect(() => {
    setLoading(true)
    invoke<EnvVar[]>('get_sandbox_env_vars', { id: sandbox.id })
      .then((v) => {
        setVars(v)
        setSaved(v)
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false))
  }, [sandbox.id])

  useEffect(() => {
    setLoadingSecrets(true)
    invoke<Secret[]>('get_sandbox_secrets', { id: sandbox.id })
      .then((s) => {
        setSecrets(s)
        setSavedSecrets(s)
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoadingSecrets(false))
  }, [sandbox.id])

  useEffect(() => {
    setLoadingGlobal(true)
    invoke<EnvVar[]>('get_global_env_vars')
      .then(setGlobalVars)
      .catch(() => {})
      .finally(() => setLoadingGlobal(false))
  }, [])

  useEffect(() => {
    setLoadingProject(true)
    invoke<EnvVar[]>('get_project_env_vars', { projectId: sandbox.project_id })
      .then(setProjectVars)
      .catch(() => {})
      .finally(() => setLoadingProject(false))
  }, [sandbox.project_id])

  useEffect(() => {
    setLoadingGlobalSecrets(true)
    invoke<Secret[]>('get_global_secrets')
      .then(setGlobalSecrets)
      .catch(() => {})
      .finally(() => setLoadingGlobalSecrets(false))
  }, [])

  useEffect(() => {
    setLoadingProjectSecrets(true)
    invoke<Secret[]>('get_project_secrets', { projectId: sandbox.project_id })
      .then(setProjectSecrets)
      .catch(() => {})
      .finally(() => setLoadingProjectSecrets(false))
  }, [sandbox.project_id])

  const dirty = !sameEnvVars(vars, saved) || !sameSecrets(secrets, savedSecrets)

  async function save() {
    setSaving(true)
    setError(null)
    try {
      if (!sameEnvVars(vars, saved)) {
        await invoke('save_sandbox_env_vars', { id: sandbox.id, vars })
        setSaved(vars)
      }
      if (!sameSecrets(secrets, savedSecrets)) {
        await invoke('save_sandbox_secrets', { id: sandbox.id, secrets })
        setSavedSecrets(secrets)
      }
      setSavedDialogOpen(true)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  function cancel() {
    setVars(saved)
    setSecrets(savedSecrets)
    setError(null)
  }

  function goToGlobalCredentials() {
    openSettings('credentials')
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between gap-4">
        <span className="text-sm font-medium text-foreground">Credentials</span>
        <SettingsActions dirty={dirty} canSave={dirty} saving={saving} error={error} onSave={save} onCancel={cancel} />
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="flex items-center justify-between gap-4 rounded-lg border border-info/40 bg-info/10 px-3 py-2.5">
        <div className="flex items-center gap-2">
          <Info className="size-3.5 shrink-0 text-info" strokeWidth={1.5} />
          <p className="text-xs text-muted-foreground">
            Global and project entries apply to more than this sandbox and can only be edited from Settings.
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={goToGlobalCredentials} className="shrink-0">
          Open global settings
        </Button>
      </div>

      <SettingsSection title="Environment Variables" divider>
        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Sandbox</span>
          {loading ? <p className="text-xs text-muted-foreground">Loading…</p> : <EnvVarEditor vars={vars} onChange={setVars} />}
        </div>

        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Global</span>
          {loadingGlobal ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <EnvVarEditor vars={globalVars} onChange={() => {}} readOnly />
          )}
        </div>

        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Project</span>
          {loadingProject ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <EnvVarEditor vars={projectVars} onChange={() => {}} readOnly />
          )}
        </div>
      </SettingsSection>

      <SettingsSection title="Secrets">
        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Sandbox</span>
          {loadingSecrets ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <SecretEditor secrets={secrets} onChange={setSecrets} />
          )}
        </div>

        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Global</span>
          {loadingGlobalSecrets ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <SecretEditor secrets={globalSecrets} onChange={() => {}} readOnly />
          )}
        </div>

        <div className="flex flex-col gap-2 py-4">
          <span className="text-xs text-muted-foreground/70">Project</span>
          {loadingProjectSecrets ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <SecretEditor secrets={projectSecrets} onChange={() => {}} readOnly />
          )}
        </div>
      </SettingsSection>

      <EnvVarsSavedDialog open={savedDialogOpen} onOpenChange={setSavedDialogOpen} />
    </div>
  )
}
