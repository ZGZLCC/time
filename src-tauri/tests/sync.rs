//! 云同步的本地合并逻辑。全程离线，不碰网络与真实数据库。
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time_lib::{
    db::{AppState, CloudEntry},
    records::{EntryState, SaveEntryInput},
};

fn temporary_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("time-sync-{}-{nonce}", std::process::id()))
}

fn state() -> (AppState, PathBuf) {
    let root = temporary_directory();
    fs::create_dir_all(&root).unwrap();
    (AppState::new(Ok(root.clone())), root)
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

fn cloud(id: &str, content: &str, updated_at: i64) -> CloudEntry {
    CloudEntry {
        id: id.to_owned(),
        start_ms: Some(1_000),
        end_ms: Some(2_000),
        start_date: None,
        end_date: None,
        content: content.to_owned(),
        tag: Some("work".to_owned()),
        state: "completed".to_owned(),
        updated_at,
        deleted_at: None,
    }
}

#[test]
fn new_entries_get_a_unique_public_id() {
    let (state, root) = state();
    let first = save(&state, "第一条", 1_000, 2_000);
    let second = save(&state, "第二条", 3_000, 4_000);
    assert_eq!(first.public_id.len(), 36, "应为 v4 UUID：{}", first.public_id);
    assert_ne!(first.public_id, second.public_id);
    assert!(first.public_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    assert_eq!(first.deleted_at, 0);
    assert!(first.updated_at > 0);
    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deleting_keeps_a_tombstone_so_it_can_sync() {
    let (state, root) = state();
    let entry = save(&state, "会被删掉", 1_000, 2_000);
    state
        .with_connection(|connection| time_lib::db::delete_entry(connection, entry.id))
        .unwrap();

    // 界面上看不到
    let visible = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert!(visible.is_empty(), "软删除后不应出现在列表里");

    // 但墓碑仍在，否则删除动作传不到其他端
    let snapshot = state.cloud_snapshot().unwrap();
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot[0].deleted_at.is_some(), "墓碑要带上删除时间");
    assert_eq!(snapshot[0].id, entry.public_id);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn merging_pulls_unknown_cloud_entries_into_the_local_database() {
    let (state, root) = state();
    let outcome = state
        .merge_cloud(vec![cloud("11111111-1111-4111-8111-111111111111", "云端来的", 5_000)])
        .unwrap();
    assert_eq!(outcome.applied, 1);

    let visible = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].content, "云端来的");
    assert_eq!(visible[0].public_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(visible[0].state, EntryState::Completed);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn newer_cloud_version_overwrites_local_and_older_one_does_not() {
    let (state, root) = state();
    let entry = save(&state, "本地原始", 1_000, 2_000);

    // 云端更旧：不该覆盖
    let stale = state
        .merge_cloud(vec![cloud(&entry.public_id, "云端旧版", 1)])
        .unwrap();
    assert_eq!(stale.applied, 0);
    let kept = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert_eq!(kept[0].content, "本地原始");

    // 云端更新：应当覆盖
    let fresh = state
        .merge_cloud(vec![cloud(&entry.public_id, "云端新版", entry.updated_at + 10_000)])
        .unwrap();
    assert_eq!(fresh.applied, 1);
    let updated = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert_eq!(updated[0].content, "云端新版");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_newer_versions_are_handed_back_for_pushing() {
    let (state, root) = state();
    let entry = save(&state, "本地较新", 1_000, 2_000);

    // 云端快照里这条是旧的，本地应当把它列进待推送
    let outcome = state
        .merge_cloud(vec![cloud(&entry.public_id, "云端旧内容", 1)])
        .unwrap();
    assert_eq!(outcome.applied, 0);
    assert_eq!(outcome.pending.len(), 1);
    assert_eq!(outcome.pending[0].id, entry.public_id);
    assert_eq!(outcome.pending[0].content, "本地较新");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_cloud_entries_are_skipped_instead_of_written() {
    let (state, root) = state();
    let mut broken = cloud("22222222-2222-4222-8222-222222222222", "已完成却缺时间", 5_000);
    broken.start_ms = None;
    broken.end_ms = None;
    let outcome = state.merge_cloud(vec![broken, cloud("", "没有编号", 5_000)]).unwrap();
    assert_eq!(outcome.applied, 0, "非法记录不应写进库");
    let visible = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert!(visible.is_empty());

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_synced_snapshot_round_trips_through_merge_without_losing_anything() {
    let (state, root) = state();
    save(&state, "第一条", 1_000, 2_000);
    save(&state, "第二条", 3_000, 4_000);
    let snapshot = state.cloud_snapshot().unwrap();
    assert_eq!(snapshot.len(), 2);

    // 模拟另一台设备：合并这份快照后应当拿到同样的两条，且没有需要推回的
    let outcome = state.merge_cloud(snapshot.clone()).unwrap();
    assert_eq!(outcome.applied, 0, "内容相同不应重复写");
    assert!(outcome.pending.is_empty(), "没有更新的版本就不该推送");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mirroring_makes_local_exactly_match_the_snapshot() {
    // 「从云端恢复」是覆盖：云端没有的本地记录要被打上删除标记
    let (state, root) = state();
    let keep = save(&state, "保留", 1_000, 2_000);
    let drop_me = save(&state, "云端没有这条", 3_000, 4_000);

    let snapshot = state.cloud_snapshot().unwrap();
    let only_keep: Vec<CloudEntry> = snapshot
        .iter()
        .filter(|entry| entry.id == keep.public_id)
        .cloned()
        .collect();

    let applied = state.mirror_cloud(only_keep).unwrap();
    assert_eq!(applied, 1);

    let visible = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert_eq!(visible.len(), 1, "云端没有的那条应当被删除");
    assert_eq!(visible[0].public_id, keep.public_id);

    // 被删的那条要留下墓碑，否则其他设备不会跟着删
    let all = state.cloud_snapshot().unwrap();
    let gone = all
        .iter()
        .find(|entry| entry.id == drop_me.public_id)
        .expect("被覆盖掉的记录仍应留在快照里作为墓碑");
    assert!(gone.deleted_at.is_some(), "覆盖删除也必须留墓碑");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mirroring_pulls_in_cloud_entries_that_local_does_not_have() {
    // 「从云端恢复」要把云端独有的记录写进本地
    let (state, root) = state();
    save(&state, "本地已有", 1_000, 2_000);
    let remote_id = "33333333-3333-4333-8333-333333333333";

    let applied = state
        .mirror_cloud(vec![cloud(remote_id, "云端独有", 5_000)])
        .unwrap();
    assert_eq!(applied, 1);

    let visible = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert_eq!(visible.len(), 1, "本地原有的那条应当被覆盖掉");
    assert_eq!(visible[0].content, "云端独有");
    assert_eq!(visible[0].public_id, remote_id);

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mirroring_keeps_tombstones_from_the_snapshot() {
    // 云端快照里的墓碑要原样落下来，否则本地会把已删记录当新记录显示
    let (state, root) = state();
    let entry = save(&state, "本地的", 1_000, 2_000);
    let mut tomb = cloud(&entry.public_id, "本地的", entry.updated_at + 10_000);
    tomb.deleted_at = Some(entry.updated_at + 10_000);

    state.mirror_cloud(vec![tomb]).unwrap();

    let visible = state.with_connection(|c| time_lib::db::list_entries(c)).unwrap();
    assert!(visible.is_empty(), "墓碑不应出现在可见列表里");
    let all = state.cloud_snapshot().unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].deleted_at.is_some());

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mirroring_skips_invalid_entries() {
    let (state, root) = state();
    let mut broken = cloud("44444444-4444-4444-8444-444444444444", "已完成却缺时间", 5_000);
    broken.start_ms = None;
    broken.end_ms = None;

    let applied = state
        .mirror_cloud(vec![broken, cloud("", "没有编号", 5_000)])
        .unwrap();
    assert_eq!(applied, 0, "非法记录不应写进库");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn expired_tombstones_are_really_deleted_and_recent_ones_are_kept() {
    let (state, root) = state();
    let now = 1_800_000_000_000i64;
    let day = 86_400_000i64;

    // 造三条：可见记录、10 天前删的（保留期内）、200 天前删的（已过期）
    let visible = save(&state, "可见记录", 1_000, 2_000);
    let recent = save(&state, "最近删的", 3_000, 4_000);
    let ancient = save(&state, "很久前删的", 5_000, 6_000);
    state
        .with_connection(|connection| {
            connection.execute(
                "UPDATE time_entries SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now - 10 * day, recent.id],
            )?;
            connection.execute(
                "UPDATE time_entries SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now - 200 * day, ancient.id],
            )?;
            Ok(())
        })
        .unwrap();

    let removed = state.purge_tombstones(now).unwrap();
    assert_eq!(removed, 1, "只应删掉过期的那条");

    // 过期那条是真的从库里没有了，不是只被过滤
    let remaining = state.cloud_snapshot().unwrap();
    let ids: Vec<&str> = remaining.iter().map(|e| e.id.as_str()).collect();
    assert!(!ids.contains(&ancient.public_id.as_str()), "过期墓碑应从库里删除");
    assert!(ids.contains(&recent.public_id.as_str()), "保留期内的墓碑必须留着");
    assert!(ids.contains(&visible.public_id.as_str()), "可见记录不该被误删");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pruning_again_removes_nothing() {
    let (state, root) = state();
    let now = 1_800_000_000_000i64;
    let entry = save(&state, "删掉的", 1_000, 2_000);
    state
        .with_connection(|connection| {
            connection.execute(
                "UPDATE time_entries SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now - 300 * 86_400_000i64, entry.id],
            )?;
            Ok(())
        })
        .unwrap();

    assert_eq!(state.purge_tombstones(now).unwrap(), 1);
    assert_eq!(state.purge_tombstones(now).unwrap(), 0, "重复清理不应再删到东西");

    drop(state);
    fs::remove_dir_all(root).unwrap();
}
