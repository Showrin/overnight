// Minimal hash-based router — no library dependency. Routes are encoded as
// `#/<screen>` or `#/<screen>/<id>` (only the "sandboxes" screen currently
// uses the id segment, for the sandbox detail page). Pairing this with the
// browser's native History API gives real back/forward navigation for free.

export type Screen = 'dashboard' | 'projects' | 'sandboxes' | 'performance-monitor' | 'settings'

export interface Route {
  screen: Screen
  sandboxId?: string
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
  const [screenPart, idPart] = hash.replace(/^#\/?/, '').split('/')
  const screen = isScreen(screenPart) ? screenPart : 'projects'
  if (screen === 'sandboxes' && idPart) {
    return { screen, sandboxId: idPart }
  }
  return { screen }
}

function routeToHash(route: Route): string {
  return route.sandboxId ? `#/${route.screen}/${route.sandboxId}` : `#/${route.screen}`
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
