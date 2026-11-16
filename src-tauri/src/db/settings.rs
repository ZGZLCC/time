use crate::error::AppError;
use rusqlite::{backup::Backup, Connection};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::Duration,
};

/// 1.3.0 起的数据库文件名。与 1.2.x 的 time.sqlite3 区分开，
/// 首次启动时把旧库复制过来再迁移，旧文件原样留着当备份。
pub(super) const DATABASE_NAME: &str = "time-v3.sqlite3";
const LEGACY_DATABASE_NAME: &str = "time.sqlite3";
const LOCATION_NAME: &str = "database-location.txt";

/// 让 Windows 路径显示成用户认得的样子。
///
/// `canonicalize()` 会返回扩展长度路径，也就是 `\\?\C:\Users\...` 这种带前缀的形式。
/// 那是给 Win32 API 用的，提示里显示出来又长又难懂。这里去掉 `\\?\`：
/// 返回是可以的——Windows 对普通绝对路径的解析完全一样，只是不再绕过 MAX_PATH 限制，
/// 而备份文件都保存在普通目录里，不涉及超长路径。
pub fn friendly_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let trimmed = text
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| text.strip_prefix(r"\\?\").map(str::to_owned));
    trimmed.unwrap_or_else(|| text.into_owned())
}

/// 解析本次要打开的数据库路径，必要时把旧库复制成新库。
///
/// 三种情况：
///   1. location 文件存在：一切以它为准，指向的文件丢了就报错，不偷偷回退；
///   2. 新库已存在但没有 location：直接用（上次复制成功但没记下位置）；
///   3. 都没有：首次运行 1.3.0，把 time.sqlite3 复制成 time-v3.sqlite3 并记下位置。
pub(super) fn prepare_path(data_dir: &Path) -> Result<PathBuf, AppError> {
    let location = data_dir.join(LOCATION_NAME);
    if location.is_file() {
        return configured_path(data_dir);
    }
    let target = data_dir.join(DATABASE_NAME);
    if target.is_file() {
        return Ok(target);
    }
    let source = data_dir.join(LEGACY_DATABASE_NAME);
    if source.is_file() {
        return promote(data_dir, &source, &target);
    }
    // 全新安装：没有任何旧库，直接用新库路径（open 会建库并跑迁移）
    Ok(target)
}

/// 把旧库原样复制到新库并记录位置。迁移由 open 之后的 migrate 负责。
///
/// 复制的是文件字节而不是用 SQLite 的备份 API：这样副本与旧库结构完全一致，
/// user_version 也原样带过来，迁移可以正常接上；旧文件一个字节都不会动。
fn promote(data_dir: &Path, source: &Path, target: &Path) -> Result<PathBuf, AppError> {
    fs::copy(source, target).map_err(|_| AppError::database())?;
    // 复制完立刻校验，副本损坏就不要切换过去
    let expected = fs::metadata(source).map(|meta| meta.len()).unwrap_or(0);
    let copied = fs::metadata(target).map(|meta| meta.len()).unwrap_or(0);
    if expected == 0 || copied != expected {
        let _ = fs::remove_file(target);
        return Err(AppError::database());
    }
    if let Err(error) = save_location(data_dir, target) {
        let _ = fs::remove_file(target);
        return Err(error);
    }
    Ok(target.to_path_buf())
}

pub(super) fn configured_path(data_dir: &Path) -> Result<PathBuf, AppError> {
    match fs::read_to_string(data_dir.join(LOCATION_NAME)) {
        Ok(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() || !path.is_file() {
                return Err(AppError::database_missing());
            }
            Ok(path)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(data_dir.join(DATABASE_NAME)),
        Err(_) => Err(AppError::database()),
    }
}

pub(super) fn database_directory(data_dir: &Path) -> Result<String, AppError> {
    configured_path(data_dir)?
        .parent()
        .map(friendly_path)
        .ok_or_else(AppError::database)
}

fn selected_folder(directory: &str) -> Result<PathBuf, AppError> {
    let path = Path::new(directory.trim());
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::validation("文件夹不存在，请检查地址"));
    }
    path.canonicalize()
        .map_err(|_| AppError::validation("无法读取所选文件夹"))
}

fn backup_into(source: &Connection, path: &Path) -> Result<Connection, AppError> {
    let result = (|| {
        let mut destination = Connection::open(path)?;
        Backup::new(source, &mut destination)?.run_to_completion(
            16,
            Duration::from_millis(10),
            None,
        )?;
        let integrity: String =
            destination.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(AppError::database());
        }
        destination.busy_timeout(Duration::from_secs(5))?;
        Ok(destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

pub(super) fn export_database(source: &Connection, directory: &str) -> Result<String, AppError> {
    let folder = selected_folder(directory)?;
    for index in 0..1000 {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{index}")
        };
        let path = folder.join(format!("Time_backup{suffix}.sqlite3"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                drop(file);
                let backup = backup_into(source, &path)?;
                drop(backup);
                return Ok(friendly_path(&path));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AppError::validation("数据库导出失败，请检查文件夹权限")),
        }
    }
    Err(AppError::validation("同名数据库备份过多，请更换文件夹"))
}

fn save_location(data_dir: &Path, path: &Path) -> Result<(), AppError> {
    let text = path
        .to_str()
        .ok_or_else(|| AppError::validation("数据库路径包含无法保存的字符"))?;
    let temporary = data_dir.join(format!("{LOCATION_NAME}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, data_dir.join(LOCATION_NAME))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|_| AppError::validation("无法保存数据库位置，请检查应用数据目录权限"))
}

/// 从备份文件恢复：用备份内容整体替换当前数据库。
///
/// 只接受不低于当前 schema 版本的备份。这样做是为了不弄丢同步信息：
/// 1.2.x 及更早的备份没有 `public_id`（跨端稳定标识）、`updated_at`、`deleted_at`，
/// 直接导进来等于把记录变成"从未同步过"的状态，还会丢掉删除墓碑，
/// 让别的设备把已删记录推回来。宁可明确报错，也不做这种有损恢复。
///
/// 当前库只在 schema 升级时由 migrate 留下 `.vN.bak` 快照；恢复这一步本身不额外备份，
/// 调用方应提醒用户先自行导出备份。
pub(super) fn restore_database(
    destination: &mut Connection,
    source_path: &Path,
    latest_version: i64,
) -> Result<i64, AppError> {
    if !source_path.is_file() {
        return Err(AppError::validation("找不到这个备份文件"));
    }
    let source = Connection::open_with_flags(
        source_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|_| AppError::validation("无法打开这个文件，请确认选的是 Time 导出的 SQLite 备份"))?;

    let version: i64 = source
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| AppError::validation("这不是 Time 的数据库备份"))?;
    // AppError 的 message 是 &'static str，所以这里用固定文案而不是拼版本号
    if version < latest_version {
        return Err(AppError::validation(
            "这个备份是旧版本导出的，缺少同步所需的记录标识，无法恢复；请选择 1.3.0 之后导出的备份",
        ));
    }
    if version > latest_version {
        return Err(AppError::validation("这个备份来自更新的版本，请先升级应用再恢复"));
    }

    let entries: i64 = source
        .query_row("SELECT count(*) FROM time_entries", [], |row| row.get(0))
        .map_err(|_| AppError::validation("备份里没有找到记录表，可能不是 Time 的备份"))?;

    // 用 SQLite 的在线备份 API 写回当前连接；备份文件本身只读，不会被改动
    Backup::new(&source, destination)
        .and_then(|backup| backup.run_to_completion(16, Duration::from_millis(10), None))
        .map_err(|_| AppError::database())?;

    let restored: i64 = destination
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| AppError::database())?;
    if restored != version {
        return Err(AppError::database());
    }
    Ok(entries)
}

pub(super) fn move_database(
    source: &mut Connection,
    data_dir: &Path,
    directory: &str,
) -> Result<String, AppError> {
    let folder = selected_folder(directory)?;
    let target = folder.join(DATABASE_NAME);
    let current = configured_path(data_dir)?
        .canonicalize()
        .map_err(|_| AppError::database_missing())?;
    if target == current {
        return Ok(friendly_path(&folder));
    }
    let reserved = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target);
    match reserved {
        Ok(file) => drop(file),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(AppError::validation(
                "所选文件夹已有 time.sqlite3，请选择空文件夹",
            ));
        }
        Err(_) => return Err(AppError::validation("无法写入所选文件夹")),
    }
    let destination = backup_into(source, &target)?;
    if let Err(error) = save_location(data_dir, &target) {
        drop(destination);
        let _ = fs::remove_file(&target);
        return Err(error);
    }
    *source = destination;
    Ok(friendly_path(&folder))
}
