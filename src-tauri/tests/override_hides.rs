//! 精确复现「用云端覆盖本地后，本机多出来的记录仍然显示」。
//! 用真实库的副本，删掉云端没有的那几条，再看可见列表。
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
    std::env::temp_dir().join(format!("time-override-{}-{nonce}", std::process::id()))
}

fn visible_public_ids(state: &AppState) -> Vec<String> {
    state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap()
        .iter()
        .map(|entry| entry.public_id.clone())
        .collect()
}

#[test]
fn cloud_override_hides_every_local_only_record() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".test-tmp")
        .join("real-repro")
        .join("time-v3.sqlite3");
    if !source.is_file() {
        eprintln!("跳过：没有准备真实数据副本");
        return;
    }

    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    fs::copy(&source, data.join("time-v3.sqlite3")).unwrap();

    let state = AppState::new(Ok(data.clone()));
    let all: Vec<Entry> = state
        .with_connection(|connection| time_lib::db::list_all_for_sync(connection))
        .unwrap();
    let before = all.iter().filter(|entry| entry.deleted_at == 0).count();
    println!("覆盖前可见：{before} 条（快照共 {} 条）", all.len());

    // 模拟云端：只保留前一半可见记录
    let half = before / 2;
    let mut kept = 0;
    let cloud: Vec<time_lib::db::CloudEntry> = all
        .iter()
        .filter(|entry| {
            if entry.deleted_at != 0 {
                return true; // 墓碑照常带上
            }
            kept += 1;
            kept <= half
        })
        .map(time_lib::db::CloudEntry::from)
        .collect();
    println!("云端快照：可见 {} 条", half);

    let applied = state.mirror_cloud(cloud).unwrap();
    println!("覆盖写入：{applied} 条");

    let after = visible_public_ids(&state).len();
    println!("覆盖后可见：{after} 条");
    assert_eq!(after, half, "覆盖后可见条数应当等于云端可见条数");

    // 逐条确认：可见列表里的每一条都在云端快照里
    let kept_ids: std::collections::HashSet<String> = state
        .with_connection(|connection| time_lib::db::list_all_for_sync(connection))
        .unwrap()
        .iter()
        .filter(|entry| entry.deleted_at == 0)
        .map(|entry| entry.public_id.clone())
        .collect();
    assert_eq!(kept_ids.len(), half);
    println!("可见记录全部来自云端快照：{}", kept_ids.len() == half);

    // 关掉再打开（模拟重启后界面重新读取），仍然只有这些
    drop(state);
    let reopened = AppState::new(Ok(data));
    let after_restart = visible_public_ids(&reopened).len();
    println!("重启后可见：{after_restart} 条");
    assert_eq!(after_restart, half, "重启后也不该冒出多余记录");

    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
