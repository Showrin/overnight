import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Check, Circle, Loader2, RotateCw, X, XCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SETUP_STEP_LABELS } from '@/lib/setupSteps'
import { useAppStore } from '@/store/useAppStore'
import { useCreateJobs, type CreateJob, type StepStatus } from '@/store/useCreateJobs'
import type { Sandbox } from './types'

const AUTO_HIDE_MS = 5000

function StepIcon({ status }: { status: StepStatus }) {
  if (status === 'running') return <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
  if (status === 'done') return <Check className="size-3.5 shrink-0 text-emerald-500" />
  if (status === 'failed') return <XCircle className="size-3.5 shrink-0 text-destructive" />
  return <Circle className="size-3.5 shrink-0 text-muted-foreground/50" />
}

function JobCard({ job }: { job: CreateJob }) {
  const dismiss = useCreateJobs((s) => s.dismiss)
  const retryCreate = useCreateJobs((s) => s.retryCreate)
  const markSetupRetried = useCreateJobs((s) => s.markSetupRetried)
  const loadNetworkPolicyPreset = useAppStore((s) => s.loadNetworkPolicyPreset)
  const loadSandboxes = useAppStore((s) => s.loadSandboxes)
  const sandbox = useAppStore((s) => s.sandboxes.find((sb) => sb.id === job.sandboxId))
  const projectName = useAppStore((s) => s.projects.find((p) => p.id === job.params.projectId)?.name)
  const [retrying, setRetrying] = useState(false)
  const [retryError, setRetryError] = useState<string | null>(null)

  useEffect(() => {
    if (job.status !== 'done') return
    const timer = setTimeout(() => dismiss(job.id), AUTO_HIDE_MS)
    return () => clearTimeout(timer)
  }, [job.status, job.id, dismiss])

  const canRetrySetup = sandbox?.status === 'running' && sandbox.setup_failures.length > 0

  async function initPolicyAndRetry(preset: 'allow-all' | 'balanced' | 'deny-all') {
    setRetrying(true)
    setRetryError(null)
    try {
      await invoke('init_sbx_policy', { preset })
      await loadNetworkPolicyPreset()
      retryCreate(job.id)
    } catch (e) {
      setRetryError(String(e))
    } finally {
      setRetrying(false)
    }
  }

  async function retrySetup() {
    if (!job.sandboxId) return
    setRetrying(true)
    setRetryError(null)
    try {
      const updated = await invoke<Sandbox>('retry_sandbox_setup', { id: job.sandboxId })
      markSetupRetried(job.id, updated)
      await loadSandboxes()
    } catch (e) {
      setRetryError(String(e))
    } finally {
      setRetrying(false)
    }
  }

  return (
    <div className="flex w-80 flex-col gap-2 rounded-lg border border-border bg-card p-3 text-sm shadow-md shadow-primary/30 ring-2 ring-primary/50">
      <div className="flex items-center gap-2">
        {job.status === 'running' ? (
          <Loader2 className="size-3.5 shrink-0 animate-spin text-primary" />
        ) : job.status === 'done' ? (
          <Check className="size-3.5 shrink-0 text-emerald-500" />
        ) : (
          <XCircle className="size-3.5 shrink-0 text-destructive" />
        )}
        <div className="flex min-w-0 flex-col">
          <span className="truncate font-medium" title={sandbox?.name ?? sandbox?.sbx_name ?? job.label}>
            {sandbox?.name ?? sandbox?.sbx_name ?? job.label}
          </span>
          <span className="truncate text-xs text-muted-foreground">
            {job.status === 'running' ? 'Creating sandbox' : job.status === 'done' ? 'Sandbox created' : 'Problem creating sandbox'}
            {projectName && ` · ${projectName}`}
          </span>
        </div>
        {job.status !== 'running' && (
          <Button size="icon-sm" variant="ghost" className="ml-auto" title="Dismiss" onClick={() => dismiss(job.id)}>
            <X className="size-3.5" />
          </Button>
        )}
      </div>
      <ul className="flex flex-col gap-1">
        {job.steps.map((step) => (
          <li key={step.key} className="flex flex-col gap-0.5">
            <div className="flex items-center gap-2">
              <StepIcon status={step.status} />
              <span className={step.status === 'pending' ? 'text-muted-foreground' : 'text-foreground'}>
                {SETUP_STEP_LABELS[step.key] ?? step.key}
              </span>
            </div>
            {step.error && (
              <p className="ml-5.5 max-h-24 overflow-auto text-xs break-all whitespace-pre-wrap text-destructive">{step.error}</p>
            )}
          </li>
        ))}
      </ul>
      {job.needsPolicyInit && (
        <div className="flex flex-col gap-2">
          <p className="text-xs text-muted-foreground">
            sbx needs a one-time network policy for this machine. Balanced allows common dev services and blocks the rest.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="outline" disabled={retrying} onClick={() => initPolicyAndRetry('allow-all')}>
              Open
            </Button>
            <Button size="sm" disabled={retrying} onClick={() => initPolicyAndRetry('balanced')}>
              Balanced
            </Button>
            <Button size="sm" variant="outline" disabled={retrying} onClick={() => initPolicyAndRetry('deny-all')}>
              Locked down
            </Button>
          </div>
        </div>
      )}
      {retryError && <p className="text-xs text-destructive">{retryError}</p>}
      {job.status === 'failed' && !job.needsPolicyInit && (
        <div className="flex justify-end gap-2">
          {canRetrySetup ? (
            <Button size="sm" variant="outline" onClick={retrySetup} disabled={retrying}>
              {retrying ? <Loader2 className="size-3.5 animate-spin" /> : <RotateCw className="size-3.5" />}
              Retry failed steps
            </Button>
          ) : (
            !sandbox?.setup_failures.length &&
            sandbox?.status !== 'running' && (
              <Button size="sm" variant="outline" onClick={() => retryCreate(job.id)}>
                <RotateCw className="size-3.5" />
                Retry create
              </Button>
            )
          )}
        </div>
      )}
    </div>
  )
}

export function CreateProgressPopout() {
  const jobs = useCreateJobs((s) => s.jobs)
  if (jobs.length === 0) return null
  return (
    <div className="flex flex-col gap-2">
      {jobs.map((job) => (
        <JobCard key={job.id} job={job} />
      ))}
    </div>
  )
}
