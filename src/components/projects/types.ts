export interface Project {
  id: string
  name: string
  repo_path: string
  kit_id: string | null
  compose_override: string | null
  created_at: number
  updated_at: number
}
