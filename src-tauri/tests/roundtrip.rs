//! 完整往返：本机 → 云端 → 本机，验证两种覆盖方向都真的覆盖。
//! 用内存里的「云端」模拟 Worker 的合并规则（按 public_id、updated_at 后写胜出）。
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time_lib::{
    db::{AppState, CloudEntry},
    records::SaveEntryInput,
};

fn staging() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("time-roundtrip-{}-{nonce}", std::process::id()))
}

fn save(state: &AppState, content: &str, start: i64, end: i64) -> time_lib::records::Entry {
    state
        .with_connection(|connection| {
            time_lib::db::save_entry(
                connection,
                SaveEntryInput {
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
        .unwrap()
}

/// 模拟 Worker：把两组快照按 public_id 合并，updated_at 更晚者胜出。
fn worker_merge(cloud: &[CloudEntry], incoming: &[CloudEntry]) -> Vec<CloudEntry> {
    let mut by_id: HashMap<String, CloudEntry> = cloud
        .iter()
        .map(|entry| (entry.id.clone(), entry.clone()))
        .collect();
    for entry in incoming {
        match by_id.get(&entry.id) {
            Some(current) if current.updated_at >= entry.updated_at => {}
            _ => {
                by_id.insert(entry.id.clone(), entry.clone());
            }
        }
    }
    by_id.into_values().collect()
}

fn visible(state: &AppState) -> Vec<String> {
    state
        .with_connection(|connection| time_lib::db::list_entries(connection))
        .unwrap()
        .iter()
        .map(|entry| entry.content.clone())
        .collect()
}

#[test]
fn local_override_pushes_222_and_does_not_pull_cloud_extras_back() {
    let root = staging();
    fs::create_dir_all(&root).unwrap();
    let state = AppState::new(Ok(root.clone()));

    // 本机 3 条
    save(&state, "本机甲", 1_000, 2_000);
    save(&state, "本机乙", 3_000, 4_000);
    save(&state, "本机丙", 5_000, 6_000);

    // 云端先有别的设备写的两条（本机没有）
    let mut cloud: Vec<CloudEntry> = vec![
        CloudEntry {
            id: "dddddddd-4444-4444-8444-dddddddddddd".to_owned(),
            start_ms: Some(10_000),
            end_ms: Some(11_000),
            start_date: None,
            end_date: None,
            content: "云端独有甲".to_owned(),
            tag: Some("work".to_owned()),
            state: "completed".to_owned(),
            updated_at: 100,
            deleted_at: None,
        },
        CloudEntry {
            id: "eeeeeeee-5555-4555-8555-eeeeeeeeeeee".to_owned(),
            start_ms: Some(12_000),
            end_ms: Some(13_000),
            start_date: None,
            end_date: None,
            content: "云端独有乙".to_owned(),
            tag: Some("work".to_owned()),
            state: "completed".to_owned(),
            updated_at: 100,
            deleted_at: None,
        },
    ];

    // ===「上传本地记录」：带 replace=1 推上去，云端直接以本地全量为准 ===
    // Worker 的 replace 分支不做合并，所以这里直接替换而不是 worker_merge
    let snapshot = state.cloud_snapshot().unwrap();
    cloud = snapshot.clone();

    let before = visible(&state);
    println!("覆盖前本机可见：{before:?}");
    // 上传成功后本地不再被改动，所以什么都不做
    let after = visible(&state);
    println!("覆盖后本机可见：{after:?}");

    assert_eq!(after.len(), 3, "本机应当仍是 3 条，云端独有的两条不该被拉回来");
    assert!(
        !after.iter().any(|content| content.starts_with("云端独有")),
        "覆盖上传后不该出现云端独有的记录，实际：{after:?}"
    );

    // 云端也应当只剩本机这三条可见
    let cloud_visible: Vec<&str> = cloud
        .iter()
        .filter(|entry| entry.deleted_at.is_none())
        .map(|entry| entry.content.as_str())
        .collect();
    println!("云端可见：{cloud_visible:?}");
    assert_eq!(cloud_visible.len(), 3, "云端可见记录应当被本机覆盖成 3 条");
    assert!(
        !cloud_visible.iter().any(|content| content.starts_with("云端独有")),
        "云端独有的记录应当被替换掉，实际：{cloud_visible:?}"
    );

    // 再走一次「立即同步」那样的合并路径：也不该把云端独有的拉回来
    // （replace 之后云端已经没有它们了）
    let merged = worker_merge(&cloud, &snapshot);
    state.mirror_cloud(merged).unwrap();
    let after_merge = visible(&state);
    println!("随后合并一次后本机可见：{after_merge:?}");
    assert_eq!(after_merge.len(), 3, "合并也不该带出多余记录：{after_merge:?}");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cloud_override_removes_local_extras_from_the_visible_list() {
    let root = staging();
    fs::create_dir_all(&root).unwrap();
    let state = AppState::new(Ok(root.clone()));

    let keep = save(&state, "云端也有", 1_000, 2_000);
    save(&state, "本机独有甲", 3_000, 4_000);
    save(&state, "本机独有乙", 5_000, 6_000);
    println!("覆盖前本机可见：{:?}", visible(&state));

    // 云端只有一条（就是 keep 那条）
    let snapshot = state.cloud_snapshot().unwrap();
    let cloud: Vec<CloudEntry> = snapshot
        .iter()
        .filter(|entry| entry.id == keep.public_id)
        .cloned()
        .collect();

    state.mirror_cloud(cloud).unwrap();
    let after = visible(&state);
    println!("覆盖后本机可见：{after:?}");
    assert_eq!(after, vec!["云端也有".to_owned()], "本机独有的两条应当消失");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_second_local_override_does_not_resurrect_deleted_records() {
    // 第一次覆盖删掉的记录，第二次覆盖不能让它复活
    let root = staging();
    fs::create_dir_all(&root).unwrap();
    let state = AppState::new(Ok(root.clone()));

    let keep = save(&state, "保留", 1_000, 2_000);
    save(&state, "会被删掉", 3_000, 4_000);

    let snapshot = state.cloud_snapshot().unwrap();
    let only_keep: Vec<CloudEntry> = snapshot
        .iter()
        .filter(|entry| entry.id == keep.public_id)
        .cloned()
        .collect();

    state.mirror_cloud(only_keep.clone()).unwrap();
    assert_eq!(visible(&state).len(), 1);

    // 第二次：本地快照里包含墓碑，推上去再镜像回来
    let second = state.cloud_snapshot().unwrap();
    let merged = worker_merge(&only_keep, &second);
    state.mirror_cloud(merged).unwrap();

    let after = visible(&state);
    println!("第二次覆盖后可见：{after:?}");
    assert_eq!(after.len(), 1, "被删掉的记录不该复活，实际：{after:?}");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}
