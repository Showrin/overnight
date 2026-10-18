import { useCallback, useMemo, useSyncExternalStore } from 'react'

// The host branch each sandbox branch is compared against on the
// Branches/Diff pages, when the user picked one other than the sandbox's
// own base branch. Stored per sandbox in localStorage as a JSON map of
// sandbox branch → host branch, so it survives restarts; removed when the
// sandbox is deleted (see pruneCompareBases).
const KEY_PREFIX = 'overnight.compareBase.'

export type CompareBases = Record<string, string>

const listeners = new Set<() => void>()

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

function emit() {
  listeners.forEach((listener) => listener())
}

function readRaw(sandboxId: string): string | null {
  try {
    return localStorage.getItem(KEY_PREFIX + sandboxId)
  } catch {
    return null
  }
}

function parse(raw: string | null): CompareBases {
  if (!raw) return {}
  try {
    const parsed: unknown = JSON.parse(raw)
    return parsed && typeof parsed === 'object' ? (parsed as CompareBases) : {}
  } catch {
    return {}
  }
}

function write(sandboxId: string, bases: CompareBases) {
  try {
    if (Object.keys(bases).length === 0) localStorage.removeItem(KEY_PREFIX + sandboxId)
    else localStorage.setItem(KEY_PREFIX + sandboxId, JSON.stringify(bases))
  } catch {
    // Storage unavailable — the choice just won't persist.
  }
  emit()
}

/** `null` clears the branch's override, falling back to the sandbox's base branch. */
export function setCompareBase(sandboxId: string, branch: string, base: string | null) {
  const bases = parse(readRaw(sandboxId))
  if (base === null) delete bases[branch]
  else bases[branch] = base
  write(sandboxId, bases)
}

/** Drops every override for one sandbox (it was deleted). */
export function clearCompareBases(sandboxId: string) {
  write(sandboxId, {})
}

/**
 * Drops overrides that point at something gone: sandbox branches not in
 * `sandboxBranches`, and/or host branches not in `hostBranches`. Pass only
 * the list(s) that were actually loaded.
 */
export function dropStaleCompareBases(
  sandboxId: string,
  { sandboxBranches, hostBranches }: { sandboxBranches?: string[]; hostBranches?: string[] }
) {
  const bases = parse(readRaw(sandboxId))
  const kept = Object.fromEntries(
    Object.entries(bases).filter(
      ([branch, base]) =>
        (!sandboxBranches || sandboxBranches.includes(branch)) && (!hostBranches || hostBranches.includes(base))
    )
  )
  if (Object.keys(kept).length !== Object.keys(bases).length) write(sandboxId, kept)
}

/** Drops overrides for sandboxes that no longer exist. */
export function pruneCompareBases(liveSandboxIds: string[]) {
  const live = new Set(liveSandboxIds)
  try {
    const stale = Object.keys(localStorage).filter(
      (key) => key.startsWith(KEY_PREFIX) && !live.has(key.slice(KEY_PREFIX.length))
    )
    if (stale.length === 0) return
    stale.forEach((key) => localStorage.removeItem(key))
  } catch {
    return
  }
  emit()
}

export function useCompareBases(sandboxId: string): CompareBases {
  const raw = useSyncExternalStore(subscribe, () => readRaw(sandboxId))
  return useMemo(() => parse(raw), [raw])
}

export function useCompareBase(sandboxId: string, branch: string): [string | null, (base: string | null) => void] {
  const base = useCompareBases(sandboxId)[branch] ?? null
  const setBase = useCallback((next: string | null) => setCompareBase(sandboxId, branch, next), [sandboxId, branch])
  return [base, setBase]
}
