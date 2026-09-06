import { invoke } from '@tauri-apps/api/core'
import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification'

// Sent via our own `notify` command rather than the plugin's `sendNotification` —
// the plugin shows the notification through notify-rust but discards the handle,
// so it can't tell us when the user clicks it. Ours waits on that click and emits
// `notification-clicked` with sandboxId, so the app can focus itself and jump to it.
export async function notify(title: string, body?: string, sandboxId?: string) {
  try {
    let granted = await isPermissionGranted()
    if (!granted) {
      const permission = await requestPermission()
      granted = permission === 'granted'
    }
    if (granted) {
      await invoke('notify', { title, body, sandboxId })
    }
  } catch {
    // Notifications are a nice-to-have — a failure here shouldn't surface as an app error.
  }
}
