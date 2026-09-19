import { isValidEnvKey } from './envVars'

export type SecretSource =
  | { kind: 'value'; value: string }
  | { kind: 'reference'; reference: string; refresh: string | null }
  | { kind: 'command'; command: string; refresh: string | null }

export type SecretTarget =
  | { kind: 'service'; service: string }
  | { kind: 'custom'; env: string; hosts: string[]; placeholder: string | null }

export interface Secret {
  target: SecretTarget
  source: SecretSource
}

// Mirrors commands.rs::KNOWN_SECRET_SERVICES.
export const KNOWN_SECRET_SERVICES = [
  'anthropic',
  'copilot',
  'cursor',
  'devin',
  'droid',
  'github',
  'google',
  'groq',
  'mistral',
  'nebius',
  'openai',
  'openrouter',
  'xai',
] as const

// The identifier a list of secrets is deduplicated by — mirrors
// commands.rs::Secret::key().
export function secretKey(secret: Secret): string {
  return secret.target.kind === 'service' ? secret.target.service : secret.target.env
}

// Mirrors commands.rs::validate_secrets — the backend is the source of
// truth, this just lets the UI reject bad input before the round trip.
export function isValidSecretEnv(env: string): boolean {
  return isValidEnvKey(env)
}

// A host must be non-empty and contain no whitespace — domains don't have
// spaces. Wildcards (`*`, `**`) are valid host characters, so no further
// charset restriction.
export function isValidHost(host: string): boolean {
  return host.trim().length > 0 && !/\s/.test(host)
}

// Splits the comma-separated host input from the "add secret" form into a
// deduplicated list of trimmed, non-empty hosts.
export function parseHosts(input: string): string[] {
  const hosts = input
    .split(',')
    .map((h) => h.trim())
    .filter((h) => h.length > 0)
  return Array.from(new Set(hosts))
}
