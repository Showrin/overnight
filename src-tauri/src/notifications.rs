//! Desktop notifications with click-to-focus support.
//!
//! `tauri-plugin-notification`'s desktop backend shows the notification via
//! `notify-rust` internally but discards the returned handle, so there is no
//! way to learn when the user clicks it. We call `notify-rust` directly here
//! instead, so we can wait for the click and focus/highlight the app.

use notify_rust::{Notification, NotificationResponse};
use tauri::{AppHandle, Manager};

/// Shows a desktop notification. If the user clicks it (as opposed to
/// dismissing it), focuses the main window and emits `notification-clicked`
/// with `sandbox_id`, if one was given.
#[tauri::command]
pub fn notify(
  app: AppHandle,
  title: String,
  body: Option<String>,
  sandbox_id: Option<String>,
) -> Result<(), String> {
  let mut notification = Notification::new();
  notification.summary(&title);
  if let Some(body) = &body {
    notification.body(body);
  }
  // Only Linux/XDG treats "default" as an invisible body-click action;
  // Windows/macOS render it as a real button and don't need it registered.
  #[cfg(not(any(target_os = "windows", target_os = "macos")))]
  notification.action("default", "default");

  let handle = notification.show().map_err(|e| e.to_string())?;

  std::thread::spawn(move || {
    let _ = handle.wait_for_response(move |response: &NotificationResponse| {
      if matches!(response, NotificationResponse::Default | NotificationResponse::Action(_)) {
        focus_main_window(&app);
        if let Some(id) = sandbox_id {
          crate::process::emit_to_webview(&app, "notification-clicked", id);
        }
      }
    });
  });

  Ok(())
}

fn focus_main_window(app: &AppHandle) {
  if let Some(window) = app.get_webview_window("main") {
    let _ = window.show();
    let _ = window.set_focus();
  }
}
