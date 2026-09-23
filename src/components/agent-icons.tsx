import type { ComponentType, SVGProps } from 'react'
import type { Agent } from '@/lib/agentHost'

function ClaudeIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" {...props}>
      {[0, 45, 90, 135, 180, 225, 270, 315].map((angle) => (
        <rect key={angle} x="11" y="2" width="2" height="7" rx="1" transform={`rotate(${angle} 12 12)`} />
      ))}
    </svg>
  )
}

function CodexIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" {...props}>
      {[0, 60, 120, 180, 240, 300].map((angle) => (
        <circle key={angle} cx="12" cy="5.5" r="2.75" transform={`rotate(${angle} 12 12)`} />
      ))}
    </svg>
  )
}

export const AGENT_ICONS: Record<Agent, ComponentType<SVGProps<SVGSVGElement>>> = {
  claude: ClaudeIcon,
  codex: CodexIcon,
}
