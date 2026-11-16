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
    std::env::temp_dir().join(format!("time-settings-{}-{nonce}", std::process::id()))
}

fn entry_count(path: &Path) -> i64 {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .query_row("SELECT count(*) FROM time_entries", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn exports_and_moves_live_database_without_overwriting_or_losing_records() {
    let root = temporary_directory();
    let data = root.join("data");
    let exports = root.join("exports");
    let first_move = root.join("first");
    let second_move = root.join("second");
    for folder in [&data, &exports, &first_move, &second_move] {
        fs::create_dir_all(folder).unwrap();
    }

    let state = AppState::new(Ok(data.clone()));
    state
        .with_connection(|connection| {
            connection.execute("INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (0, 60000, '原记录', 'work', 'completed')", [])?;
            Ok(())
        })
        .unwrap();
    let exported = PathBuf::from(state.export_database(exports.to_str().unwrap()).unwrap());
    let another = PathBuf::from(state.export_database(exports.to_str().unwrap()).unwrap());
    assert_ne!(exported, another);
    assert_eq!(entry_count(&exported), 1);
    assert_eq!(entry_count(&another), 1);

    let active = state.move_database(first_move.to_str().unwrap()).unwrap();
    // 返回的是去掉 Windows 扩展长度前缀的路径，所以要拿 friendly_path 比对
    assert_eq!(active, friendly_path(&first_move.canonicalize().unwrap()));
    assert_eq!(state.database_directory().unwrap(), active);
    assert!(state.move_database(exports.to_str().unwrap()).is_ok());
    state
        .with_connection(|connection| {
            connection.execute("INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (60000, 120000, '新记录', 'work', 'completed')", [])?;
            Ok(())
        })
        .unwrap();
    drop(state);

    assert_eq!(entry_count(&data.join("time-v3.sqlite3")), 1);
    let reopened = AppState::new(Ok(data.clone()));
    assert_eq!(
        reopened.database_directory().unwrap(),
        friendly_path(&exports.canonicalize().unwrap())
    );
    assert_eq!(
        reopened
            .with_connection(|connection| Ok(connection.query_row(
                "SELECT count(*) FROM time_entries",
                [],
                |row| row.get::<_, i64>(0)
            )?))
            .unwrap(),
        2
    );
    assert!(reopened
        .move_database(first_move.to_str().unwrap())
        .is_err());
    reopened
        .move_database(second_move.to_str().unwrap())
        .unwrap();
    drop(reopened);

    let final_state = AppState::new(Ok(data.clone()));
    assert_eq!(
        final_state.database_directory().unwrap(),
        friendly_path(&second_move.canonicalize().unwrap())
    );
    assert_eq!(entry_count(&second_move.join("time-v3.sqlite3")), 2);
    drop(final_state);
    fs::remove_file(second_move.join("time-v3.sqlite3")).unwrap();
    let missing = AppState::new(Ok(data));
    assert!(missing.with_connection(|_| Ok(())).is_err());
    assert!(!second_move.join("time-v3.sqlite3").exists());
    drop(missing);
    fs::remove_dir_all(root).unwrap();
}
