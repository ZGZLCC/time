use super::{migrate_with, MIGRATIONS};
use rusqlite::Connection;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_database_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "time-migration-{}-{nonce}.sqlite3",
        std::process::id()
    ))
}

#[test]
fn failed_migration_rolls_back_and_preserves_backup() {
    let path = temp_database_path();
    let mut connection = Connection::open(&path).unwrap();
    migrate_with(&mut connection, Some(&path), &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (1, 2, '保留', 'work', 'completed')",
            [],
        )
        .unwrap();

    let failure = migrate_with(
        &mut connection,
        Some(&path),
        &[
            (1, MIGRATIONS[0].1),
            (2, "CREATE TABLE pending_change (id INTEGER); INVALID SQL;"),
        ],
    )
    .unwrap_err();
    assert_eq!(failure.code, crate::error::ErrorCode::Migration);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT content FROM time_entries WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
        "保留"
    );
    assert!(connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'pending_change'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap()
        == 0);

    let backup_path = path.with_file_name(format!(
        "{}.v1.bak",
        path.file_name().unwrap().to_string_lossy()
    ));
    assert!(backup_path.exists());
    let backup = Connection::open(&backup_path).unwrap();
    assert_eq!(
        backup
            .query_row("SELECT content FROM time_entries WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
        "保留"
    );
    drop(backup);
    drop(connection);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(backup_path);
}

#[test]
fn upgrade_preserves_entries_and_converts_running_to_draft() {
    let path = temp_database_path();
    let mut connection = Connection::open(&path).unwrap();
    migrate_with(&mut connection, Some(&path), &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO time_entries (start_ms, content, state) VALUES (1000, '', 'running')",
            [],
        )
        .unwrap();
    connection.execute("INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (2000, 3000, '已完成', 'work', 'completed')", []).unwrap();
    migrate_with(&mut connection, Some(&path), MIGRATIONS).unwrap();
    let entries = super::list_entries(&mut connection).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|entry| entry.state == crate::records::EntryState::Draft
            && entry.start_ms == Some(1000)
            && entry.end_ms.is_none()));
    assert!(entries
        .iter()
        .any(|entry| entry.state == crate::records::EntryState::Completed
            && entry.content == "已完成"));
    drop(connection);
    let _ = std::fs::remove_file(path);
}
