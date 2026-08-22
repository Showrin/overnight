export interface Project {
  id: string
  name: string
  repo_path: string
  plans_path: string | null
  dev_server_port: number | null
  extra_clone_paths: string[]
  created_at: number
  updated_at: number
}
