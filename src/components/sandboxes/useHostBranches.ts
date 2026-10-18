import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { dropStaleCompareBases } from '@/lib/compareBase'

export interface HostBranches {
  branches: string[] | null
  error: string | null
  reload: () => void
}

/**
 * The host repo's branches, listed once per page and shared by every
 * picker on it. Overrides pointing at a host branch that no longer exists
 * are dropped once the list loads, so a deleted branch doesn't leave the
 * pages stuck on a git error.
 */
export function useHostBranches(sandboxId: string): HostBranches {
  const [branches, setBranches] = useState<string[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [version, setVersion] = useState(0)

  useEffect(() => {
    let cancelled = false
    invoke<string[]>('list_host_branches', { id: sandboxId })
      .then((list) => {
        if (cancelled) return
        setBranches(list)
        setError(null)
        dropStaleCompareBases(sandboxId, { hostBranches: list })
      })
      .catch((e) => !cancelled && setError(String(e)))
    return () => {
      cancelled = true
    }
  }, [sandboxId, version])

  const reload = useCallback(() => setVersion((v) => v + 1), [])
  return { branches, error, reload }
}
