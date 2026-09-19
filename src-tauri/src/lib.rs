mod backup;
mod commands;
mod daemon_log;
mod db;
mod git;
mod jira;
mod notifications;
mod process;
mod providers;
mod restore;
mod sbx;
mod skills;

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
      commands::save_skill_folders,
      commands::get_global_env_vars,
      commands::save_global_env_vars,
      commands::get_project_env_vars,
      commands::save_project_env_vars,
      commands::get_sandbox_env_vars,
      commands::save_sandbox_env_vars,
      commands::get_global_secrets,
      commands::save_global_secrets,
      commands::get_project_secrets,
      commands::save_project_secrets,
      commands::get_sandbox_secrets,
      commands::save_sandbox_secrets,
      commands::get_platform,
      commands::get_default_terminal_host,
      commands::save_default_terminal_host,
      commands::start_plan_session,
      commands::sbx_health_check,
      commands::init_sbx_policy,
      commands::get_network_policy_settings,
      commands::save_default_network_policy_preset,
      commands::add_network_rule,
      commands::remove_network_rule,
      commands::get_sandbox_network_rules,
      commands::add_sandbox_network_rule,
      commands::remove_sandbox_network_rule,
      commands::set_sandbox_network_preset_override,
      commands::set_anthropic_api_key,
      commands::list_sandboxes,
      commands::adopt_orphan_sandboxes,
      commands::create_sandbox,
      commands::stop_sandbox,
      commands::start_sandbox,
      commands::delete_sandbox,
      commands::get_sandbox_usage,
      commands::get_backup_interval_minutes,
      commands::save_backup_interval_minutes,
      commands::get_auto_backup_enabled,
      commands::save_auto_backup_enabled,
      commands::list_active_backups,
      commands::list_active_operations,
      commands::backup_sandbox_now,
      commands::list_backups,
      commands::delete_sandbox_backup,
      commands::delete_sandbox_backups_for_sandbox,
      commands::restore_backup,
      commands::sync_sandbox_plans,
      commands::get_sandbox_branch_info,
      commands::open_sandbox_vscode,
      commands::open_sandbox_terminal,
      commands::git_sync_sandbox,
      commands::get_sandbox_diff,
      commands::get_branch_commits,
      commands::open_path_in_explorer,
      commands::open_backups_root_folder,
      commands::get_host_stats,
      commands::get_host_stats_history,
      commands::get_sandbox_resource_usage,
      commands::get_sandbox_resource_history,
      commands::list_command_log,
      commands::count_command_log,
      commands::list_command_log_operations,
      commands::get_daemon_log_path,
      commands::save_daemon_log_path,
      commands::read_daemon_log,
      notifications::notify,
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
          "host_metrics",
          "command_log",
          "sandbox_backups",
        ];
        for table in table_names {
          match conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get::<_, i64>(0)) {
            Ok(count) => log::info!("db: {table} has {count} row(s)"),
            Err(e) => log::warn!("db: couldn't read {table}: {e}"),
          }
        }
      }
      app.manage(std::sync::Mutex::new(backup::BackupState::default()));
      app.manage(std::sync::Mutex::new(restore::RestoreState::default()));
      tauri::async_runtime::spawn(backup::run_scheduler(app.handle().clone(), pool.clone()));
      app.manage(pool);
      app.manage(std::sync::Mutex::new(sbx::HostMonitor::new()));
      app.manage(std::sync::Mutex::new(sbx::SandboxMonitor::new()));

      if let Some(window) = app.get_webview_window("main") {
        if let Ok(Some(monitor)) = window.primary_monitor() {
          let screen_size = monitor.size();
          let width = (screen_size.width as f64 * 0.7) as u32;
          let height = (screen_size.height as f64 * 0.7) as u32;
          let _ = window.set_size(tauri::PhysicalSize::new(width, height));
          let _ = window.center();
        }
        let _ = window.show();
      }

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
