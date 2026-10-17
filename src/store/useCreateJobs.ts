import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Sandbox, SandboxMode } from '@/components/sandboxes/types'
import type { NetworkRuleDecision } from '@/lib/networkPolicy'
import { notify } from '@/lib/notify'
import { useAppStore } from './useAppStore'

export type StepStatus = 'pending' | 'running' | 'done' | 'failed'

export interface CreateStep {
  key: string
  status: StepStatus
  error?: string
}

export interface CreateParams {
  projectId: string
  mode: SandboxMode
  name: string | null
  permissionMode: string | null
  agent: string
  kitId: string | null
  networkPresetOverride: string | null
  networkRules: { decision: NetworkRuleDecision; host: string }[]
}

export interface CreateJob {
  id: string
  label: string
  params: CreateParams
  sandboxId: string | null
  steps: CreateStep[]
  status: 'running' | 'done' | 'failed'
  error: string | null
  needsPolicyInit: boolean
}

interface CreateProgressEvent {
  job_id: string
  sandbox_id: string
  step: string
  status: StepStatus
  error: string | null
}

interface CreateJobsStore {
  jobs: CreateJob[]
  startCreate: (params: CreateParams, label: string) => void
  retryCreate: (id: string) => void
  dismiss: (id: string) => void
  applyProgress: (event: CreateProgressEvent) => void
  markSetupRetried: (id: string, sandbox: Sandbox) => void
}

function initialSteps(params: CreateParams): CreateStep[] {
  const keys = ['create', 'ready', 'permission_mode', 'ports', 'env_vars', 'secrets']
  if (params.mode === 'clone') keys.push('git_origin')
  if (params.networkPresetOverride || params.networkRules.length > 0) keys.push('network_rules')
  return keys.map((key) => ({ key, status: 'pending' }))
}

function upsertStep(steps: CreateStep[], key: string, patch: Partial<CreateStep>): CreateStep[] {
  return steps.some((s) => s.key === key)
    ? steps.map((s) => (s.key === key ? { ...s, ...patch } : s))
    : [...steps, { key, status: 'pending', ...patch }]
}

export const useCreateJobs = create<CreateJobsStore>((set, get) => {
  function patchJob(id: string, patch: (job: CreateJob) => Partial<CreateJob>) {
    set((state) => ({ jobs: state.jobs.map((j) => (j.id === id ? { ...j, ...patch(j) } : j)) }))
  }

  async function run(id: string) {
    const job = get().jobs.find((j) => j.id === id)
    if (!job) return
    const { params } = job
    try {
      const sandbox = await invoke<Sandbox>('create_sandbox', {
        projectId: params.projectId,
        mode: params.mode,
        name: params.name,
        permissionMode: params.permissionMode,
        agent: params.agent,
        kitId: params.kitId,
        progressId: id,
      })
      patchJob(id, () => ({ sandboxId: sandbox.id }))
      if (params.networkPresetOverride || params.networkRules.length > 0) {
        patchJob(id, (j) => ({ steps: upsertStep(j.steps, 'network_rules', { status: 'running' }) }))
        try {
          if (params.networkPresetOverride) {
            await invoke('set_sandbox_network_preset_override', { id: sandbox.id, preset: params.networkPresetOverride })
          }
          for (const rule of params.networkRules) {
            await invoke('add_sandbox_network_rule', { id: sandbox.id, decision: rule.decision, host: rule.host })
          }
          patchJob(id, (j) => ({ steps: upsertStep(j.steps, 'network_rules', { status: 'done' }) }))
        } catch (e) {
          patchJob(id, (j) => ({ steps: upsertStep(j.steps, 'network_rules', { status: 'failed', error: String(e) }) }))
        }
      }
      const failed = sandbox.setup_failures.length > 0 || get().jobs.find((j) => j.id === id)?.steps.some((s) => s.status === 'failed')
      // Steps the backend skipped never got an event.
      patchJob(id, (j) => ({
        status: failed ? 'failed' : 'done',
        steps: j.steps.filter((s) => s.status !== 'pending'),
      }))
      notify(
        failed ? 'Sandbox started with setup errors' : 'Sandbox started',
        failed ? `${job.label} is running, but some setup steps failed.` : `${job.label} is up and running.`,
        sandbox.id,
      )
    } catch (e) {
      const message = String(e)
      const needsPolicyInit = message.includes("network policy hasn't been initialized")
      patchJob(id, (j) => ({
        status: 'failed',
        error: message,
        needsPolicyInit,
        steps: j.steps.some((s) => s.status === 'failed')
          ? j.steps
          : upsertStep(j.steps, 'create', { status: 'failed', error: message }),
      }))
      if (!needsPolicyInit) notify('Sandbox failed to start', message)
    } finally {
      useAppStore.getState().loadSandboxes()
    }
  }

  return {
    jobs: [],

    startCreate(params, label) {
      const id = crypto.randomUUID()
      const job: CreateJob = {
        id,
        label,
        params,
        sandboxId: null,
        steps: initialSteps(params),
        status: 'running',
        error: null,
        needsPolicyInit: false,
      }
      set((state) => ({ jobs: [...state.jobs, job] }))
      void run(id)
    },

    retryCreate(id) {
      patchJob(id, (j) => ({ status: 'running', error: null, needsPolicyInit: false, sandboxId: null, steps: initialSteps(j.params) }))
      void run(id)
    },

    dismiss(id) {
      set((state) => ({ jobs: state.jobs.filter((j) => j.id !== id) }))
    },

    applyProgress(event) {
      patchJob(event.job_id, (j) => ({
        sandboxId: event.sandbox_id,
        steps: upsertStep(j.steps, event.step, { status: event.status, error: event.error ?? undefined }),
      }))
    },

    markSetupRetried(id, sandbox) {
      const failedKeys = new Set(sandbox.setup_failures.map((f) => f.step as string))
      patchJob(id, (j) => {
        const steps = j.steps.map((s) => {
          if (s.key === 'network_rules') return s
          const failure = sandbox.setup_failures.find((f) => f.step === s.key)
          return failure ? { ...s, status: 'failed' as const, error: failure.error } : s.status === 'failed' ? { ...s, status: 'done' as const, error: undefined } : s
        })
        const anyFailed = failedKeys.size > 0 || steps.some((s) => s.status === 'failed')
        return { steps, status: anyFailed ? 'failed' : 'done' }
      })
    },
  }
})

let listening = false

export function initCreateJobs() {
  if (listening) return
  listening = true
  listen<CreateProgressEvent>('sandbox-create-progress', (event) => {
    useCreateJobs.getState().applyProgress(event.payload)
  })
}
