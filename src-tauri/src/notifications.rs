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
  let product_name = app.config().product_name.clone().unwrap_or_else(|| "Overnight".into());
  notification.appname(&product_name);
  notification.summary(&title);
  if let Some(body) = &body {
    notification.body(body);
  }
  // Only Linux/XDG treats "default" as an invisible body-click action;
  // Windows/macOS render it as a real button and don't need it registered.
  #[cfg(not(any(target_os = "windows", target_os = "macos")))]
  notification.action("default", "default");

  // Windows toasts derive both the shown name and icon from `app_id` (the AUMID), not from
  // `appname`. Leaving it unset falls back to `Toast::POWERSHELL_APP_ID`. Only set it once the
  // app is installed (i.e. its Start Menu shortcut has registered this AUMID); in a dev build
  // there's no such registration, so setting an arbitrary id would make toasts silently fail.
  #[cfg(windows)]
  if let Ok(exe) = tauri::utils::platform::current_exe() {
    if let Some(exe_dir) = exe.parent() {
      if let Some(id) = windows_app_id(&app.config().identifier, exe_dir) {
        notification.app_id(&id);
      }
    }
  }

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
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
  }
}

/// Windows AUMID to use for the toast, or `None` to keep the (unregistered-in-dev) fallback.
/// Mirrors `tauri-plugin-notification`'s own dev/prod detection so dev builds keep working.
/// Kept free of `#[cfg(windows)]` (unlike its only call site) so it's unit-testable everywhere.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_app_id(identifier: &str, exe_dir: &std::path::Path) -> Option<String> {
  let in_target_debug_or_release = matches!(exe_dir.file_name().and_then(|n| n.to_str()), Some("debug" | "release"))
    && matches!(exe_dir.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()), Some("target"));
  if in_target_debug_or_release {
    None
  } else {
    Some(identifier.to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  #[test]
  fn dev_debug_build_keeps_fallback() {
    assert_eq!(windows_app_id("com.sbhustles.overnight", Path::new("C:/project/target/debug")), None);
  }

  #[test]
  fn dev_release_build_keeps_fallback() {
    assert_eq!(windows_app_id("com.sbhustles.overnight", Path::new("C:/project/target/release")), None);
  }

  #[test]
  fn installed_build_uses_identifier() {
    assert_eq!(
      windows_app_id("com.sbhustles.overnight", Path::new("C:/Program Files/Overnight")),
      Some("com.sbhustles.overnight".to_string())
    );
  }
}
