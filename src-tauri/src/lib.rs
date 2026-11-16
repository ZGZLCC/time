pub mod commands;
pub mod day;
pub mod db;
pub mod error;
pub mod export;
pub mod records;
pub mod week;

use tauri::Manager;

#[tauri::command]
fn greet() -> String {
    "Rust 后端连接正常".into()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = app
                .path()
                .app_data_dir()
                .map_err(|_| error::AppError::database())
                .and_then(|directory| {
                    std::fs::create_dir_all(&directory).map_err(|_| error::AppError::database())?;
                    Ok(directory)
                });
            app.manage(db::AppState::new(directory));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::save_entry,
            commands::list_entries,
            commands::delete_entry,
            commands::start_punch,
            commands::list_pending,
            commands::get_week,
            commands::get_day_totals,
            commands::get_all_totals,
            commands::default_export_directory,
            commands::save_week_image,
            commands::database_directory,
            commands::export_database,
            commands::move_database,
            commands::clear_entries,
            commands::purge_tombstones,
            commands::restore_database,
            commands::pick_backup_file,
            commands::cloud_snapshot,
            commands::merge_cloud,
            commands::mirror_cloud
        ])
        .run(tauri::generate_context!())
        .expect("启动桌面应用失败");
}
