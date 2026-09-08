// Minimal hash-based router — no library dependency. Routes are encoded as
// `#/<screen>`, `#/<screen>/<id>` (sandbox detail), `#/<screen>/<id>/branches`
// (branches page), or `#/<screen>/<id>/branches/<branch>` (branch diff page —
// the branch name is percent-encoded into a single segment so names
// containing "/" round-trip safely). Pairing this with the browser's native
// History API gives real back/forward navigation for free.

export type Screen = 'dashboard' | 'projects' | 'sandboxes' | 'performance-monitor' | 'settings'

export interface Route {
  screen: Screen
  sandboxId?: string
  branches?: boolean
  branch?: string
}

const SCREENS: readonly Screen[] = ['dashboard', 'projects', 'sandboxes', 'performance-monitor', 'settings']

function isScreen(value: string): value is Screen {
  return (SCREENS as readonly string[]).includes(value)
}

/**
 * Parses a `location.hash` value into a Route. Anything empty or
 * unrecognized falls back to the Projects screen.
 */
export function parseRoute(hash: string): Route {
  const [screenPart, idPart, viewPart, branchPart] = hash.replace(/^#\/?/, '').split('/')
  const screen = isScreen(screenPart) ? screenPart : 'projects'
  if (screen === 'sandboxes' && idPart) {
    if (viewPart === 'branches') {
      return { screen, sandboxId: idPart, branches: true, branch: branchPart ? decodeURIComponent(branchPart) : undefined }
    }
    return { screen, sandboxId: idPart }
  }
  return { screen }
}

function routeToHash(route: Route): string {
  if (!route.sandboxId) return `#/${route.screen}`
  if (!route.branches) return `#/${route.screen}/${route.sandboxId}`
  const branchSegment = route.branch ? `/${encodeURIComponent(route.branch)}` : ''
  return `#/${route.screen}/${route.sandboxId}/branches${branchSegment}`
}

/**
 * Pushes a new history entry for `route` (no-op if it's already the current
 * hash), enabling native browser back/forward between screens and sandbox
 * detail pages.
 */
export function pushRoute(route: Route): void {
  const hash = routeToHash(route)
  if (window.location.hash !== hash) {
    window.history.pushState(null, '', hash)
  }
}
