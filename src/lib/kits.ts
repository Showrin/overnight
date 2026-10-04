import type { Project } from '@/components/projects/types'

export interface Kit {
  id: string
  name: string
  spec: string
  is_global: boolean
  project_ids: string[]
  created_at: number
  updated_at: number
}

export function defaultKitId(kits: Kit[], project: Project | undefined): string | null {
  if (project?.kit_id && kits.some((k) => k.id === project.kit_id)) return project.kit_id
  return kits.find((k) => k.is_global)?.id ?? null
}

export function kitScopeLabel(kit: Kit): string {
  if (kit.is_global) return 'Global'
  const count = kit.project_ids.length
  if (count === 0) return 'Not applied'
  return count === 1 ? '1 project' : `${count} projects`
}

export function starterSpec(slug: string): string {
  return `schemaVersion: "2"
kind: mixin
name: ${slug}
description: Tools installed when the sandbox is created

setup:
  install:
    - command: "apt-get update && apt-get install -y jq"
      description: Install jq
    # Pin Node.js with nvm. Base image sets NPM_CONFIG_PREFIX, which nvm rejects.
    # - command: "curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash"
    #   user: "1000"
    # - command: "bash -c 'unset NPM_CONFIG_PREFIX; . $HOME/.nvm/nvm.sh && nvm install 22 && nvm alias default 22'"
    #   user: "1000"
    # - command: |
    #     cat >> /etc/sandbox-persistent.sh <<'EOF'
    #     export NVM_DIR="$HOME/.nvm"
    #     unset NPM_CONFIG_PREFIX
    #     [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
    #     EOF
    #   user: "1000"
    # Pin Python with uv (preinstalled in base image):
    # - command: "uv python install 3.12 --default"
    #   user: "1000"
`
}
