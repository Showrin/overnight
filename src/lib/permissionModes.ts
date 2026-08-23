export const PERMISSION_MODES = ['plan', 'default', 'acceptEdits', 'bypassPermissions'] as const
export type PermissionMode = (typeof PERMISSION_MODES)[number]
