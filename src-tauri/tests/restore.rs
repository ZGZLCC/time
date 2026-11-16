//! 从备份文件恢复的行为。全程在临时目录，不碰真实数据库。
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use time_lib::db::{friendly_path, AppState};

fn temporary_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("time-restore-{}-{nonce}", std::process::id()))
}

fn save(state: &AppState, content: &str, start: i64, end: i64) {
    state
        .with_connection(|connection| {
            time_lib::db::save_entry(
                connection,
                time_lib::records::SaveEntryInput {
                    id: None,
                    start_ms: Some(start),
                    end_ms: Some(end),
                    start_date: None,
                    end_date: None,
                    content: content.to_owned(),
                    tag: Some("work".to_owned()),
                },
            )
        })
        .unwrap();
}

fn visible_count(state: &AppState) -> usize {
    state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap()
        .len()
}

/// 出错时把 AppError 的 code 与 message 都打出来，
/// 否则界面上只会看到一句通用文案，定位不到是哪一步。
fn describe(error: &time_lib::error::AppError) -> String {
    format!("{:?} / {}", error.code, error.message)
}

#[test]
fn restoring_immediately_after_export_works() {
    // 用户的实际操作：备份一份，紧接着就用它恢复
    let root = temporary_directory();
    let data = root.join("data");
    let exports = root.join("exports");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&exports).unwrap();

    let state = AppState::new(Ok(data));
    save(&state, "记录一", 1_000, 2_000);
    save(&state, "记录二", 3_000, 4_000);

    let backup = state.export_database(exports.to_str().unwrap()).unwrap();
    let restored = state
        .restore_database(&backup)
        .unwrap_or_else(|error| panic!("恢复失败：{}", describe(&error)));
    assert_eq!(restored, 2);
    assert_eq!(visible_count(&state), 2);

    // 用户可能连点两次，第二次也必须成功
    let again = state
        .restore_database(&backup)
        .unwrap_or_else(|error| panic!("第二次恢复失败：{}", describe(&error)));
    assert_eq!(again, 2);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exported_paths_hide_the_windows_verbatim_prefix() {
    // canonicalize 在 Windows 上返回 \\?\C:\... 这种扩展长度路径，
    // 直接显示给用户又长又难懂；friendly_path 负责去掉前缀。
    assert_eq!(
        friendly_path(Path::new(r"\\?\C:\Users\me\Downloads\Time_backup.sqlite3")),
        r"C:\Users\me\Downloads\Time_backup.sqlite3"
    );
    // UNC 路径要还原成 \\server\share 而不是 server\share
    assert_eq!(
        friendly_path(Path::new(r"\\?\UNC\server\share\Time_backup.sqlite3")),
        r"\\server\share\Time_backup.sqlite3"
    );
    // 普通路径原样返回，不误伤
    assert_eq!(
        friendly_path(Path::new(r"C:\Users\me\Downloads\Time_backup.sqlite3")),
        r"C:\Users\me\Downloads\Time_backup.sqlite3"
    );
    assert_eq!(friendly_path(Path::new("/home/me/Time_backup.sqlite3")), "/home/me/Time_backup.sqlite3");
}

#[test]
fn exported_backup_path_is_a_plain_absolute_path() {
    let root = temporary_directory();
    let data = root.join("data");
    let exports = root.join("exports");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&exports).unwrap();

    let state = AppState::new(Ok(data));
    save(&state, "一条记录", 1_000, 2_000);
    let path = state.export_database(exports.to_str().unwrap()).unwrap();

    assert!(
        !path.starts_with(r"\\?\"),
        "导出提示里不该出现扩展长度前缀，实际是：{path}"
    );
    assert!(path.ends_with("Time_backup.sqlite3"), "实际是：{path}");
    // 去掉前缀后的路径仍然可用
    assert!(PathBuf::from(&path).is_file());

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn restoring_a_backup_replaces_current_records() {
    let root = temporary_directory();
    let data = root.join("data");
    let exports = root.join("exports");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&exports).unwrap();

    // 造一份只有 1 条记录的备份
    let state = AppState::new(Ok(data.clone()));
    save(&state, "备份里的记录", 1_000, 2_000);
    let backup = state.export_database(exports.to_str().unwrap()).unwrap();

    // 再往当前库加两条，使它与备份不同
    save(&state, "后来加的 1", 3_000, 4_000);
    save(&state, "后来加的 2", 5_000, 6_000);
    assert_eq!(visible_count(&state), 3);

    // 恢复备份：应整体替换回 1 条
    let restored = state.restore_database(&backup).unwrap();
    assert_eq!(restored, 1, "返回值应是恢复的记录条数");
    assert_eq!(visible_count(&state), 1, "当前记录应被备份整体替换");

    let content = state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap()[0]
        .content
        .clone();
    assert_eq!(content, "备份里的记录");

    // 备份文件本身只读，不该被动过
    assert!(PathBuf::from(&backup).is_file());

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_exported_backup_itself_is_a_valid_restorable_database() {
    // 恢复失败可能出在备份文件本身：先独立校验它，再看恢复。
    let root = temporary_directory();
    let data = root.join("data");
    let exports = root.join("exports");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&exports).unwrap();

    let state = AppState::new(Ok(data));
    save(&state, "记录一", 1_000, 2_000);
    save(&state, "记录二", 3_000, 4_000);
    let backup = state.export_database(exports.to_str().unwrap()).unwrap();

    // 独立打开备份文件检查
    let check = rusqlite::Connection::open_with_flags(
        &backup,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap_or_else(|error| panic!("备份文件打不开：{error}"));
    let version: i64 = check
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let count: i64 = check
        .query_row("SELECT count(*) FROM time_entries", [], |row| row.get(0))
        .unwrap();
    let integrity: String = check
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    drop(check);

    assert_eq!(
        version,
        time_lib::db::latest_schema_version(),
        "备份的 schema 版本应当是最新版"
    );
    assert_eq!(count, 2, "备份里的记录数应当与导出时一致");
    assert_eq!(integrity, "ok", "备份文件应当通过完整性检查");

    // 再去恢复
    let restored = state
        .restore_database(&backup)
        .unwrap_or_else(|error| panic!("恢复失败：{}", describe(&error)));
    assert_eq!(restored, 2);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn restoring_a_backup_keeps_the_database_usable() {
    // 恢复之后再正常读写一次，确认库没被弄坏（界面上的报错也可能是这一步暴露的）
    let root = temporary_directory();
    let data = root.join("data");
    let exports = root.join("exports");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&exports).unwrap();

    let state = AppState::new(Ok(data));
    save(&state, "备份里的", 1_000, 2_000);
    let backup = state.export_database(exports.to_str().unwrap()).unwrap();

    state
        .restore_database(&backup)
        .unwrap_or_else(|error| panic!("恢复失败：{}", describe(&error)));

    // 恢复后还能写入与读取
    save(&state, "恢复后新增", 5_000, 6_000);
    assert_eq!(visible_count(&state), 2);

    // 关掉再打开同一个目录（模拟重启），库仍然可用
    drop(state);
    let reopened = AppState::new(Ok(root.join("data")));
    assert_eq!(visible_count(&reopened), 2, "重启后应还能读到恢复的数据");
    drop(reopened);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_schema_backups_are_rejected_with_a_clear_message() {
    let root = temporary_directory();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();

    // 造一个 v2 的老备份：没有 public_id / updated_at / deleted_at
    let legacy = root.join("legacy.sqlite3");
    {
        let connection = rusqlite::Connection::open(&legacy).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE time_entries (
                    id INTEGER PRIMARY KEY,
                    start_ms INTEGER, end_ms INTEGER,
                    start_date TEXT, end_date TEXT,
                    content TEXT NOT NULL DEFAULT '', tag TEXT,
                    state TEXT NOT NULL
                 ) STRICT;
                 INSERT INTO time_entries (start_ms, end_ms, content, tag, state)
                 VALUES (1000, 2000, '老备份', 'work', 'completed');
                 PRAGMA user_version = 2;",
            )
            .unwrap();
    }

    let state = AppState::new(Ok(data));
    save(&state, "当前记录", 1_000, 2_000);

    let error = state.restore_database(legacy.to_str().unwrap()).unwrap_err();
    assert_eq!(error.code, time_lib::error::ErrorCode::Validation);
    assert!(
        error.message.contains("旧版本"),
        "应当明确说明是旧版本备份，实际是：{}",
        error.message
    );
    // 拒绝之后当前数据必须完好
    assert_eq!(visible_count(&state), 1, "拒绝恢复时不能动当前数据");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn restoring_a_missing_or_foreign_file_fails_without_touching_data() {
    let root = temporary_directory();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let state = AppState::new(Ok(data));
    save(&state, "当前记录", 1_000, 2_000);

    assert!(state
        .restore_database(root.join("没有这个文件.sqlite3").to_str().unwrap())
        .is_err());

    // 一个不是数据库的普通文件
    let foreign = root.join("随便一个文件.sqlite3");
    fs::write(&foreign, b"this is not a sqlite database").unwrap();
    assert!(state.restore_database(foreign.to_str().unwrap()).is_err());

    assert_eq!(visible_count(&state), 1, "失败时不能动当前数据");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}
