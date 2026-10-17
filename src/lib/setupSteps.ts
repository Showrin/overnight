// Mirrors db/models.rs::SetupStep.
export type SetupStep = 'permission_mode' | 'env_vars' | 'secrets' | 'git_origin' | 'compose_override' | 'agent_config'

export interface SetupFailure {
  step: SetupStep
  error: string
}

export const SETUP_STEP_LABELS: Record<string, string> = {
  create: 'Create sandbox',
  ready: 'Wait until ready',
  permission_mode: 'Permission mode',
  env_vars: 'Environment variables',
  secrets: 'Secrets',
  git_origin: 'Git origin',
  compose_override: 'Compose override',
  agent_config: 'Hooks & output styles',
  network_rules: 'Network rules',
}
