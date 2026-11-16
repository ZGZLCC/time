use crate::{
    db::{self, AppState, CloudEntry, MergeOutcome},
    error::AppError,
    export,
    records::{Entry, SaveEntryInput},
    week::{WeekTotals, WeekView},
};
use tauri::{Manager, State};

#[tauri::command]
pub fn save_entry(state: State<'_, AppState>, input: SaveEntryInput) -> Result<Entry, AppError> {
    state.with_connection(|connection| db::save_entry(connection, input))
}

#[tauri::command]
pub fn list_entries(state: State<'_, AppState>) -> Result<Vec<Entry>, AppError> {
    state.with_connection(db::list_entries)
}

#[tauri::command]
pub fn delete_entry(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    state.with_connection(|connection| db::delete_entry(connection, id))
}

#[tauri::command]
pub fn start_punch(state: State<'_, AppState>) -> Result<Entry, AppError> {
    state.with_connection(db::start_punch)
}

#[tauri::command]
pub fn list_pending(state: State<'_, AppState>) -> Result<Vec<Entry>, AppError> {
    state.with_connection(db::list_pending)
}

#[tauri::command]
pub fn get_week(state: State<'_, AppState>, date: String) -> Result<WeekView, AppError> {
    state.with_connection(|connection| db::get_week(connection, &date))
}

#[tauri::command]
pub fn get_day_totals(state: State<'_, AppState>, date: String) -> Result<WeekTotals, AppError> {
    state.with_connection(|connection| db::get_day_totals(connection, &date))
}

#[tauri::command]
pub fn get_all_totals(state: State<'_, AppState>) -> Result<WeekTotals, AppError> {
    state.with_connection(db::get_all_totals)
}

/// 默认导出目录（通常是「下载」文件夹）。
///
/// Tauri 在 Windows 上返回的是扩展长度路径（`\\?\C:\Users\...`），
/// 直接显示给用户又长又难懂，这里统一去掉前缀。
#[tauri::command]
pub fn default_export_directory(app: tauri::AppHandle) -> Result<String, AppError> {
    app.path()
        .download_dir()
        .map(|path| db::friendly_path(&path))
        .map_err(|_| AppError::validation("无法读取下载文件夹"))
}

#[tauri::command]
pub fn save_week_image(
    directory: String,
    start_date: String,
    end_date: String,
    png_bytes: Vec<u8>,
) -> Result<String, AppError> {
    export::save_week_png(&directory, &start_date, &end_date, &png_bytes)
}

#[tauri::command]
pub fn database_directory(state: State<'_, AppState>) -> Result<String, AppError> {
    state.database_directory()
}

#[tauri::command]
pub fn export_database(state: State<'_, AppState>, directory: String) -> Result<String, AppError> {
    state.export_database(&directory)
}

#[tauri::command]
pub fn move_database(state: State<'_, AppState>, directory: String) -> Result<String, AppError> {
    state.move_database(&directory)
}

/// 清空全部记录（软删除）。
#[tauri::command]
pub fn clear_entries(state: State<'_, AppState>) -> Result<usize, AppError> {
    state.clear_entries()
}

/// 真正删除超过保留期的墓碑，返回删除条数。
#[tauri::command]
pub fn purge_tombstones(state: State<'_, AppState>, now: i64) -> Result<usize, AppError> {
    state.purge_tombstones(now)
}

/// 从备份文件恢复，返回恢复的记录条数。
#[tauri::command]
pub fn restore_database(state: State<'_, AppState>, path: String) -> Result<i64, AppError> {
    state.restore_database(&path)
}

/// 弹出选择备份文件的对话框，返回所选路径；用户取消时返回 None。
#[tauri::command]
pub async fn pick_backup_file(app: tauri::AppHandle) -> Result<Option<String>, AppError> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .add_filter("Time 数据库备份", &["sqlite3", "db", "sqlite"])
        .blocking_pick_file();
    Ok(picked
        .and_then(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned()))
}

/// 同步用：本地全部记录的云端格式快照。
#[tauri::command]
pub fn cloud_snapshot(state: State<'_, AppState>) -> Result<Vec<CloudEntry>, AppError> {
    state.cloud_snapshot()
}

/// 同步用：用云端快照整体覆盖本地，返回写入条数。
#[tauri::command]
pub fn mirror_cloud(state: State<'_, AppState>, incoming: Vec<CloudEntry>) -> Result<i64, AppError> {
    state.mirror_cloud(incoming)
}
/// 同步用：合并云端快照，返回写回条数与待推送记录。
#[tauri::command]
pub fn merge_cloud(state: State<'_, AppState>, incoming: Vec<CloudEntry>) -> Result<MergeOutcome, AppError> {
    state.merge_cloud(incoming)
}
