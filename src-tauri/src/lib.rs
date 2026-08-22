mod commands;
mod db;
mod docker;
mod jira;
mod process;
mod providers;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_shell::init())
    .invoke_handler(tauri::generate_handler![
      commands::list_tasks,
      commands::create_task,
      commands::get_task,
      commands::update_task_status,
      commands::delete_task,
      commands::start_session,
      commands::get_session,
      commands::list_sessions_for_task,
      commands::end_session,
      commands::save_jira_config,
      commands::get_jira_config,
      commands::sync_jira_issues,
      commands::list_jira_issues,
      commands::list_projects,
      commands::create_project,
      commands::update_project,
      commands::delete_project,
      commands::get_settings,
      commands::save_settings,
      commands::start_plan_session,
      commands::docker_health_check,
      commands::list_sandboxes,
      commands::create_sandbox,
      commands::stop_sandbox,
      commands::start_sandbox,
      commands::delete_sandbox,
      commands::get_sandbox_metrics,
      commands::get_sandbox_usage,
      commands::stream_sandbox_logs,
      commands::open_sandbox_vscode,
      commands::open_sandbox_terminal,
    ])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      let pool = db::init_pool(app.handle())?;
      {
        let conn = pool.get()?;
        let table_names = [
          "tasks",
          "sessions",
          "activity",
          "metrics",
          "container_metrics",
          "settings",
          "jira_issues",
          "projects",
          "sandboxes",
        ];
        for table in table_names {
          let count: i64 = conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))?;
          log::info!("db: {table} has {count} row(s)");
        }
      }
      app.manage(pool);

      let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
      let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

      TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&tray_menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
          "show" => {
            if let Some(window) = app.get_webview_window("main") {
              let _ = window.show();
              let _ = window.set_focus();
            }
          }
          "quit" => {
            app.exit(0);
          }
          _ => {}
        })
        .on_tray_icon_event(|tray, event| {
          if let TrayIconEvent::DoubleClick { .. } = event {
            let app = tray.app_handle();
            if let Some(window) = app.get_webview_window("main") {
              let _ = window.show();
              let _ = window.set_focus();
            }
          }
        })
        .build(app)?;

      Ok(())
    })
    .on_window_event(|window, event| {
      if let WindowEvent::CloseRequested { api, .. } = event {
        window.hide().unwrap();
        api.prevent_close();
      }
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
