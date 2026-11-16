use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time_lib::{
    db,
    error::ErrorCode,
    records::{EntryState, SaveEntryInput},
};

fn temp_database_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "time-records-{}-{nonce}.sqlite3",
        std::process::id()
    ))
}

fn input(id: Option<i64>, start_ms: i64, end_ms: i64, content: &str) -> SaveEntryInput {
    SaveEntryInput {
        id,
        start_ms: Some(start_ms),
        end_ms: Some(end_ms),
        start_date: None,
        end_date: None,
        content: content.to_owned(),
        tag: Some("work".to_owned()),
    }
}

#[test]
fn saves_edits_deletes_and_persists_without_overlaps() {
    let path = temp_database_path();
    let mut connection = db::open(&path).unwrap();
    let first = db::save_entry(&mut connection, input(None, 0, 100, "任务一")).unwrap();
    let second = db::save_entry(&mut connection, input(None, 100, 200, "任务二")).unwrap();

    let overlap = db::save_entry(&mut connection, input(None, 50, 150, "冲突记录")).unwrap_err();
    assert_eq!(overlap.code, ErrorCode::Overlap);
    assert_eq!(overlap.conflicting_entry_id, Some(first.id));

    let edited = db::save_entry(
        &mut connection,
        input(Some(first.id), 0, 100, "\u{3000}修改后\u{3000}"),
    )
    .unwrap();
    assert_eq!(edited.content, "修改后");

    let invalid = db::save_entry(&mut connection, input(None, 200, 200, "时间错误")).unwrap_err();
    assert_eq!(invalid.code, ErrorCode::Validation);
    assert_eq!(db::list_entries(&mut connection).unwrap().len(), 2);

    db::delete_entry(&mut connection, second.id).unwrap();
    drop(connection);

    let mut reopened = db::open(&path).unwrap();
    let entries = db::list_entries(&mut reopened).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].content, "修改后");
    drop(reopened);
    let _ = std::fs::remove_file(path);
}

#[test]
fn punch_creates_independent_start_only_drafts_after_restart() {
    let path = temp_database_path();
    let mut connection = db::open(&path).unwrap();
    let first = db::start_punch(&mut connection).unwrap();
    let second = db::start_punch(&mut connection).unwrap();
    assert_ne!(first.id, second.id);
    drop(connection);

    let mut reopened = db::open(&path).unwrap();
    let restored = db::list_pending(&mut reopened).unwrap();
    assert_eq!(restored.len(), 2);
    assert!(restored
        .iter()
        .all(|entry| entry.state == EntryState::Draft && entry.end_ms.is_none()));
    let completed = db::save_entry(
        &mut reopened,
        input(
            Some(first.id),
            first.start_ms.unwrap(),
            first.start_ms.unwrap() + 60_000,
            "阶段三验证",
        ),
    )
    .unwrap();
    assert_eq!(completed.state, EntryState::Completed);
    assert_eq!(completed.content, "阶段三验证");
    assert_eq!(db::list_pending(&mut reopened).unwrap().len(), 1);
    drop(reopened);
    let _ = std::fs::remove_file(path);
}

#[test]
fn partial_manual_record_keeps_date_and_stays_out_of_completed_history() {
    let path = temp_database_path();
    let mut connection = db::open(&path).unwrap();
    let draft = db::save_entry(
        &mut connection,
        SaveEntryInput {
            id: None,
            start_ms: None,
            end_ms: None,
            start_date: Some("2026-09-26".to_owned()),
            end_date: Some("2026-09-26".to_owned()),
            content: "".to_owned(),
            tag: None,
        },
    )
    .unwrap();
    assert_eq!(draft.state, EntryState::Draft);
    assert_eq!(draft.start_date.as_deref(), Some("2026-09-26"));
    drop(connection);

    let mut reopened = db::open(&path).unwrap();
    assert_eq!(db::list_pending(&mut reopened).unwrap().len(), 1);
    assert!(db::get_week(&mut reopened, "2026-09-26")
        .unwrap()
        .entries
        .is_empty());
    drop(reopened);
    let _ = std::fs::remove_file(path);
}
