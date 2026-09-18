export interface EnvVar {
  key: string
  value: string
}

const ENV_KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/

// Mirrors commands.rs::validate_env_vars — the backend is the source of
// truth, this just lets the UI reject bad input before the round trip.
export function isValidEnvKey(key: string): boolean {
  return ENV_KEY_PATTERN.test(key)
}
