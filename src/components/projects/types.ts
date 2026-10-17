export interface Project {
  id: string
  name: string
  repo_path: string
  plans_path: string | null
  dev_server_port: number | null
  kit_id: string | null
  ports: number[]
  browser_urls: string[]
  created_at: number
  updated_at: number
}
