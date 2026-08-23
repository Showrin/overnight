import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification'

export async function notify(title: string, body?: string) {
  try {
    let granted = await isPermissionGranted()
    if (!granted) {
      const permission = await requestPermission()
      granted = permission === 'granted'
    }
    if (granted) {
      sendNotification({ title, body })
    }
  } catch {
    // Notifications are a nice-to-have — a failure here shouldn't surface as an app error.
  }
}
