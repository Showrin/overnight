import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Info } from 'lucide-react'
import { EnvVarEditor } from '@/components/settings/EnvVarEditor'
import { EnvVarsSavedDialog } from '@/components/settings/EnvVarsSavedDialog'
import { SettingsActions } from '@/components/settings/SettingsLayout'
import { Button } from '@/components/ui/button'
import type { EnvVar } from '@/lib/envVars'
import type { Route } from '@/lib/router'
import type { Sandbox } from './types'

function sameEnvVars(a: EnvVar[], b: EnvVar[]): boolean {
  return a.length === b.length && a.every((v, i) => v.key === b[i].key && v.value === b[i].value)
}

export function SandboxCredentialsTab({ sandbox, navigate }: { sandbox: Sandbox; navigate: (route: Route) => void }) {
  const [vars, setVars] = useState<EnvVar[]>([])
  const [saved, setSaved] = useState<EnvVar[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [globalVars, setGlobalVars] = useState<EnvVar[]>([])
  const [loadingGlobal, setLoadingGlobal] = useState(true)

  const [projectVars, setProjectVars] = useState<EnvVar[]>([])
  const [loadingProject, setLoadingProject] = useState(true)

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

  const dirty = !sameEnvVars(vars, saved)

  async function save() {
    setSaving(true)
    setError(null)
    try {
      await invoke('save_sandbox_env_vars', { id: sandbox.id, vars })
      setSaved(vars)
      setSavedDialogOpen(true)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  function cancel() {
    setVars(saved)
    setError(null)
  }

  function goToGlobalCredentials() {
    navigate({ screen: 'settings', settingsTab: 'credentials' })
  }

  return (
    <div className="flex flex-col gap-6">
      {/* Section 1: this sandbox's own credentials — the only thing editable here. */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between gap-4">
          <div className="flex flex-col gap-1">
            <span className="text-sm font-medium text-foreground">Sandbox credentials</span>
            <span className="text-xs text-muted-foreground">Applied to this sandbox only, on top of global and project credentials.</span>
          </div>
          <SettingsActions dirty={dirty} canSave={dirty} saving={saving} error={error} onSave={save} onCancel={cancel} />
        </div>
        {error && <p className="text-sm text-destructive">{error}</p>}
        {loading ? <p className="text-sm text-muted-foreground">Loading…</p> : <EnvVarEditor vars={vars} onChange={setVars} />}
      </div>

      <div className="border-t border-border" />

      {/* Section 2: everything else this sandbox inherits — view only. */}
      <div className="flex flex-col gap-4">
        <span className="text-sm font-medium text-foreground">Other credentials</span>

        <div className="flex items-center justify-between gap-4 rounded-lg border border-info/40 bg-info/10 px-3 py-2.5">
          <div className="flex items-center gap-2">
            <Info className="size-3.5 shrink-0 text-info" strokeWidth={1.5} />
            <p className="text-xs text-muted-foreground">
              Global and project credentials apply to more than this sandbox and can only be edited from Settings.
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={goToGlobalCredentials} className="shrink-0">
            Open global settings
          </Button>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs text-muted-foreground/70">Global</span>
          {loadingGlobal ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <EnvVarEditor vars={globalVars} onChange={() => {}} readOnly />
          )}
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs text-muted-foreground/70">Project</span>
          {loadingProject ? (
            <p className="text-xs text-muted-foreground">Loading…</p>
          ) : (
            <EnvVarEditor vars={projectVars} onChange={() => {}} readOnly />
          )}
        </div>
      </div>

      <EnvVarsSavedDialog open={savedDialogOpen} onOpenChange={setSavedDialogOpen} />
    </div>
  )
}
