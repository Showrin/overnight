import type { Project } from '@/components/projects/types'

export interface ComposeOverride {
  id: string
  name: string
  content: string
  is_global: boolean
  project_ids: string[]
  created_at: number
  updated_at: number
}

export function defaultComposeOverrideId(items: ComposeOverride[], project: Project | undefined): string | null {
  if (project?.compose_override_id && items.some((o) => o.id === project.compose_override_id)) {
    return project.compose_override_id
  }
  return items.find((o) => o.is_global)?.id ?? null
}

export const STARTER_COMPOSE_OVERRIDE = `services:
  web:
    ports:
      - "3000:3000"
`
