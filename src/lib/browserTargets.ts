export interface PortMapping {
  sandbox_port: number
  host_port: number
}

export interface BrowserTarget {
  label: string
  url: string
}

// `{port}` in a project browser URL is replaced with the first published host port.
export function browserTargets(ports: PortMapping[], hostPort: number | null, browserUrls: string[]): BrowserTarget[] {
  const mappings = ports.length > 0 ? ports : hostPort != null ? [{ sandbox_port: hostPort, host_port: hostPort }] : []
  const targets: BrowserTarget[] = mappings.map((p) => ({
    label: p.sandbox_port === p.host_port ? `localhost:${p.host_port}` : `localhost:${p.host_port} → ${p.sandbox_port}`,
    url: `http://localhost:${p.host_port}`,
  }))
  const firstHostPort = mappings[0]?.host_port
  for (const template of browserUrls) {
    if (template.includes('{port}') && firstHostPort == null) continue
    const url = firstHostPort == null ? template : template.replaceAll('{port}', String(firstHostPort))
    targets.push({ label: url, url })
  }
  return targets
}

export function parsePorts(input: string): number[] | null {
  const parts = input.split(',').map((p) => p.trim()).filter((p) => p.length > 0)
  const ports = parts.map(Number)
  if (ports.some((p) => !Number.isInteger(p) || p < 1 || p > 65535)) return null
  return Array.from(new Set(ports))
}
