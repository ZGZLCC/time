//! 迁移 4（重建表以允许空内容）在真实数据上的验证：
//! 逐条比对迁移前后的记录，确认一条不少、一个字段没错。
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time_lib::{db::AppState, records::Entry};

fn staging() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("time-mig4-{}-{nonce}", std::process::id()))
}

/// 用同一套 SQL 文本读库，迁移前后才可比：
/// 若一边用 rusqlite 原文、一边用 Rust 枚举的 Debug 输出，
/// 会看到 "completed" 与 Completed 这种假差异。
const ROW_SQL: &str = "SELECT id, start_ms, end_ms, start_date, end_date, content, tag, state, public_id, updated_at, deleted_at FROM time_entries ORDER BY id";

fn read_rows(path: &PathBuf) -> Vec<String> {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mut statement = connection.prepare(ROW_SQL).unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok(format!(
                "{}|{:?}|{:?}|{:?}|{:?}|{}|{:?}|{:?}|{}|{}|{}",
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    rows
}

#[test]
fn migration_four_keeps_every_row_of_the_real_database() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".test-tmp")
        .join("real-repro")
        .join("time-v3.sqlite3");
    if !source.is_file() {
        eprintln!("跳过：没有准备真实数据副本");
        return;
    }

    // 先直接读原库（此时还是 v3，没跑迁移），拿到迁移前的快照
    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    fs::copy(&source, data.join("time-v3.sqlite3")).unwrap();

    let raw = rusqlite::Connection::open(data.join("time-v3.sqlite3")).unwrap();
    let before_version: i64 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    drop(raw);

    let before = read_rows(&data.join("time-v3.sqlite3"));
    println!("迁移前：v{before_version}，{} 条", before.len());

    // 打开应用，让它跑迁移
    let state = AppState::new(Ok(data.clone()));
    println!("迁移后：{} 条", before.len());

    // 迁移后用同一套 SQL 读一遍，两边才可比
    drop(state);
    let after = read_rows(&data.join("time-v3.sqlite3"));

    assert_eq!(before.len(), after.len(), "迁移后条数必须一致");
    for (index, (old, new)) in before.iter().zip(after.iter()).enumerate() {
        if old != new {
            println!("第 {index} 条不一致：");
            println!("  迁移前：{old}");
            println!("  迁移后：{new}");
        }
        assert_eq!(old, new, "第 {index} 条记录在迁移前后不一致");
    }
    println!("逐条比对：{} 条全部一致", before.len());

    // 版本号与完整性
    let check = rusqlite::Connection::open(data.join("time-v3.sqlite3")).unwrap();
    let version: i64 = check
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let integrity: String = check
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    drop(check);
    assert_eq!(version, 4, "迁移后版本应为 4");
    assert_eq!(integrity, "ok", "迁移后完整性应为 ok");

    // 迁移后必须能存空内容的已完成记录——这正是本次修复的目的
    let reopened = AppState::new(Ok(data.clone()));
    reopened
        .with_connection(|connection| {
            time_lib::db::save_entry(
                connection,
                time_lib::records::SaveEntryInput {
                    id: None,
                    start_ms: Some(999_000),
                    end_ms: Some(999_500),
                    start_date: None,
                    end_date: None,
                    content: String::new(),
                    tag: Some("work".to_owned()),
                },
            )
        })
        .expect("迁移后应当能保存空内容的已完成记录");
    println!("迁移后空内容保存：通过");

    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
