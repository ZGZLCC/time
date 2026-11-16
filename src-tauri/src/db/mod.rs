mod entries;
mod punch;
mod settings;
mod week;

#[cfg(test)]
mod tests;

pub use entries::{
    clear_entries, delete_entry, list_all_for_sync, list_entries, merge_cloud_entries, mirror_cloud_entries,
    purge_tombstones, save_entry, CloudEntry, MergeOutcome,
};
pub use punch::{list_pending, start_punch};
pub use settings::friendly_path;
pub use week::{get_all_totals, get_day_totals, get_week};

use crate::error::AppError;
use rusqlite::{backup::Backup, Connection, TransactionBehavior};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_initial.sql")),
    (2, include_str!("../../migrations/0002_partial_entries.sql")),
    (3, include_str!("../../migrations/0003_sync_columns.sql")),
    (4, include_str!("../../migrations/0004_empty_content.sql")),
];

pub struct AppState {
    directory: PathBuf,
    connection: Result<Mutex<Connection>, AppError>,
}

impl AppState {
    pub fn new(directory: Result<PathBuf, AppError>) -> Self {
        match directory {
            Ok(directory) => Self {
                // prepare_path 负责首次启动时把旧库复制成新库；旧文件一个字节都不动
                connection: settings::prepare_path(&directory)
                    .and_then(|path| open(&path))
                    .map(Mutex::new),
                directory,
            },
            Err(error) => Self {
                directory: PathBuf::new(),
                connection: Err(error),
            },
        }
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let connection = self.connection.as_ref().map_err(Clone::clone)?;
        let mut connection = connection.lock().map_err(|_| AppError::database())?;
        operation(&mut connection)
    }

    pub fn database_directory(&self) -> Result<String, AppError> {
        self.connection.as_ref().map_err(Clone::clone)?;
        settings::database_directory(&self.directory)
    }

    pub fn export_database(&self, directory: &str) -> Result<String, AppError> {
        self.with_connection(|connection| settings::export_database(connection, directory))
    }

    pub fn move_database(&self, directory: &str) -> Result<String, AppError> {
        self.with_connection(|connection| {
            settings::move_database(connection, &self.directory, directory)
        })
    }

    /// 清空全部记录（软删除，保留墓碑供同步使用）。
    pub fn clear_entries(&self) -> Result<usize, AppError> {
        self.with_connection(clear_entries)
    }

    /// 真正删除超过保留期的墓碑，返回删除条数。
    pub fn purge_tombstones(&self, now: i64) -> Result<usize, AppError> {
        self.with_connection(|connection| entries::purge_tombstones(connection, now))
    }

    /// 从备份文件恢复，返回恢复的记录条数。
    pub fn restore_database(&self, path: &str) -> Result<i64, AppError> {
        self.with_connection(|connection| {
            settings::restore_database(connection, Path::new(path), latest_schema_version())
        })
    }

    /// 同步用：把本地全部记录（含墓碑）转成云端格式，供推送给 Worker。
    pub fn cloud_snapshot(&self) -> Result<Vec<CloudEntry>, AppError> {
        self.with_connection(|connection| {
            Ok(list_all_for_sync(connection)?
                .iter()
                .map(CloudEntry::from)
                .collect())
        })
    }

    /// 同步用：合并云端快照到本地，并返回需要推回云端的更晚版本。
    pub fn merge_cloud(&self, incoming: Vec<CloudEntry>) -> Result<MergeOutcome, AppError> {
        self.with_connection(|connection| merge_cloud_entries(connection, &incoming))
    }

    /// 用云端快照整体覆盖本地，并返回写入条数。
    /// 快照里没有的记录会打上删除标记（墓碑随后会被推回云端）。
    pub fn mirror_cloud(&self, incoming: Vec<CloudEntry>) -> Result<i64, AppError> {
        self.with_connection(|connection| mirror_cloud_entries(connection, incoming))
    }
}

/// 仅供测试：直接对连接执行一段操作，便于构造与检查库内状态。
#[cfg(test)]
pub fn with_test_connection<T>(
    state: &AppState,
    operation: impl FnOnce(&mut Connection) -> Result<T, AppError>,
) -> Result<T, AppError> {
    state.with_connection(operation)
}

pub fn open(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| AppError::database())?;
    }
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    migrate(&mut connection, Some(path))?;
    Ok(connection)
}

pub fn open_memory() -> Result<Connection, AppError> {
    let mut connection = Connection::open_in_memory()?;
    migrate(&mut connection, None)?;
    Ok(connection)
}

fn migrate(connection: &mut Connection, path: Option<&Path>) -> Result<(), AppError> {
    migrate_with(connection, path, MIGRATIONS)
}

/// 某个库当前的 schema 版本，供恢复备份时校验。
pub fn schema_version(connection: &Connection) -> Result<i64, AppError> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| AppError::migration())
}

/// 最新 schema 版本。恢复备份时要求备份文件不小于这个版本。
pub fn latest_schema_version() -> i64 {
    MIGRATIONS.last().map_or(0, |(version, _)| *version)
}

fn migrate_with(
    connection: &mut Connection,
    path: Option<&Path>,
    migrations: &[(i64, &str)],
) -> Result<(), AppError> {
    let current: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| AppError::migration())?;
    let latest = migrations.last().map_or(0, |(version, _)| *version);
    if current < 0 || current > latest {
        return Err(AppError::migration());
    }

    let pending: Vec<_> = migrations
        .iter()
        .filter(|(version, _)| *version > current)
        .collect();
    if pending.is_empty() {
        return Ok(());
    }

    let backup = if current > 0 {
        Some(create_backup(
            connection,
            path.ok_or_else(AppError::migration)?,
            current,
        )?)
    } else {
        None
    };

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AppError::migration())?;
    let mut applied = current;
    for (version, sql) in pending {
        if *version != applied + 1 {
            return Err(AppError::migration());
        }
        transaction
            .execute_batch(sql)
            .map_err(|_| AppError::migration())?;
        transaction
            .pragma_update(None, "user_version", version)
            .map_err(|_| AppError::migration())?;
        applied = *version;
    }
    transaction.commit().map_err(|_| AppError::migration())?;

    if let Some(backup) = backup {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn create_backup(connection: &Connection, path: &Path, version: i64) -> Result<PathBuf, AppError> {
    let file_name = path
        .file_name()
        .ok_or_else(AppError::migration)?
        .to_string_lossy();
    let backup_path = path.with_file_name(format!("{file_name}.v{version}.bak"));
    if backup_path.exists() {
        let backup = Connection::open(&backup_path).map_err(|_| AppError::migration())?;
        let backup_version: i64 = backup
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| AppError::migration())?;
        if backup_version != version {
            return Err(AppError::migration());
        }
        return Ok(backup_path);
    }

    let result = (|| {
        let mut destination = Connection::open(&backup_path).map_err(|_| AppError::migration())?;
        Backup::new(connection, &mut destination)
            .and_then(|backup| backup.run_to_completion(16, Duration::from_millis(10), None))
            .map_err(|_| AppError::migration())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&backup_path);
    }
    result?;
    Ok(backup_path)
}
