//! 空内容的「已完成」记录：保存、读取、重叠检查，以及同步快照与合并。
//! 这些都是 1.3.3 起才允许的组合（原来是草稿）。
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time_lib::{
    db::AppState,
    records::{EntryState, SaveEntryInput},
};

fn describe(error: &time_lib::error::AppError) -> String {
    format!("{:?} / {}", error.code, error.message)
}

fn staging() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("time-empty-{}-{nonce}", std::process::id()))
}

fn input(content: &str, start: i64, end: i64, tag: Option<&str>) -> SaveEntryInput {
    SaveEntryInput {
        id: None,
        start_ms: Some(start),
        end_ms: Some(end),
        start_date: None,
        end_date: None,
        content: content.to_owned(),
        tag: tag.map(str::to_owned),
    }
}

#[test]
fn saving_a_completed_entry_with_empty_content_works() {
    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let state = AppState::new(Ok(data));

    let entry = state
        .with_connection(|connection| time_lib::db::save_entry(connection, input("", 1_000, 2_000, Some("work"))))
        .unwrap_or_else(|error| panic!("空内容保存失败：{}", describe(&error)));

    assert_eq!(entry.state, EntryState::Completed, "空内容也应是已完成");
    assert_eq!(entry.content, "", "内容应保持为空");
    assert!(!entry.public_id.is_empty(), "应当生成 public_id");

    // 正常的使用流程里紧接着会读一次列表
    let listed = state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap_or_else(|error| panic!("保存后读取失败：{}", describe(&error)));
    assert_eq!(listed.len(), 1);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn several_empty_content_entries_at_different_times_are_all_fine() {
    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let state = AppState::new(Ok(data));

    // 连续记多条「没写做了什么」的记录，是这次改动后很常见的用法
    for index in 0..5 {
        let start = 1_000 + index * 10_000;
        state
            .with_connection(|connection| {
                time_lib::db::save_entry(connection, input("", start, start + 5_000, Some("work")))
            })
            .unwrap_or_else(|error| panic!("第 {index} 条空内容保存失败：{}", describe(&error)));
    }

    let listed = state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap();
    assert_eq!(listed.len(), 5);
    assert!(listed.iter().all(|entry| entry.content.is_empty()));

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_content_entries_survive_the_sync_snapshot_and_merge() {
    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let state = AppState::new(Ok(data));

    state
        .with_connection(|connection| time_lib::db::save_entry(connection, input("", 1_000, 2_000, Some("work"))))
        .unwrap();
    state
        .with_connection(|connection| time_lib::db::save_entry(connection, input("有内容", 3_000, 4_000, Some("work"))))
        .unwrap();

    let snapshot = state
        .cloud_snapshot()
        .unwrap_or_else(|error| panic!("取快照失败：{}", describe(&error)));
    assert_eq!(snapshot.len(), 2);

    // 另一台设备合并这份快照
    let outcome = state
        .merge_cloud(snapshot.clone())
        .unwrap_or_else(|error| panic!("合并失败：{}", describe(&error)));
    assert_eq!(outcome.applied, 0, "内容相同不应重复写");

    // 空内容那条还能改
    let target = snapshot
        .iter()
        .find(|entry| entry.content.is_empty())
        .expect("快照里应当有空内容那条");
    let mut edited = target.clone();
    edited.content = "补上的内容".to_owned();
    edited.updated_at += 1_000;
    state
        .merge_cloud(vec![edited])
        .unwrap_or_else(|error| panic!("改空内容那条失败：{}", describe(&error)));

    let after = state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap();
    assert!(after.iter().any(|entry| entry.content == "补上的内容"));

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_content_entries_appear_in_week_statistics() {
    // 空内容记录是已完成，应当计入周统计与重叠检查
    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let state = AppState::new(Ok(data));

    state
        .with_connection(|connection| time_lib::db::save_entry(connection, input("", 1_000, 2_000, Some("work"))))
        .unwrap();

    let totals = state
        .with_connection(|connection| time_lib::db::get_all_totals(connection))
        .unwrap_or_else(|error| panic!("统计失败：{}", describe(&error)));
    assert!(totals.work_ms > 0, "空内容的已完成记录应当计入统计");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

/// 另一台设备（例如网页版）建的「有起止时间和标签、只是没写内容」的记录必须能合并进本机。
///
/// 合并路径以前把它当成非法数据静默跳过，而同一个库里的快照因为 `updated_at` 相同
/// 本来就写不进去，看不出区别——只有空库 + 一条云端记录才测得到这个方向。
#[test]
fn empty_content_record_from_another_device_merges_into_the_local_database() {
    let root = staging();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let state = AppState::new(Ok(data));

    let incoming = time_lib::db::CloudEntry {
        id: "11111111-1111-4111-8111-111111111111".to_owned(),
        start_ms: Some(1_000),
        end_ms: Some(2_000),
        start_date: None,
        end_date: None,
        content: String::new(),
        tag: Some("work".to_owned()),
        state: "completed".to_owned(),
        updated_at: 1_500,
        deleted_at: None,
    };

    let outcome = state
        .merge_cloud(vec![incoming])
        .unwrap_or_else(|error| panic!("合并失败：{}", describe(&error)));
    assert_eq!(outcome.applied, 1, "空内容的已完成记录也应当合并进本机");

    let listed = state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap();
    assert_eq!(listed.len(), 1, "合并后应当出现在记录列表里");
    assert_eq!(listed[0].content, "");
    assert_eq!(listed[0].state, EntryState::Completed);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}
