// Mirrors db/models.rs::SetupStep.
export type SetupStep = 'permission_mode' | 'ports' | 'env_vars' | 'secrets' | 'git_origin' | 'compose_override'

export interface SetupFailure {
  step: SetupStep
  error: string
}

export const SETUP_STEP_LABELS: Record<string, string> = {
  create: 'Create sandbox',
  ready: 'Wait until ready',
  permission_mode: 'Permission mode',
  ports: 'Ports',
  env_vars: 'Environment variables',
  secrets: 'Secrets',
  git_origin: 'Git origin',
  compose_override: 'Compose override',
  network_rules: 'Network rules',
}
