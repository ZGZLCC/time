use time_lib::{
    day::assemble_day,
    db,
    records::{Entry, EntryState, Tag},
    week::{assemble_week, week_range},
};

const MINUTE_MS: i64 = 60_000;

fn completed(id: i64, start_ms: i64, end_ms: i64, tag: Tag) -> Entry {
    Entry {
        id,
        public_id: format!("00000000-0000-4000-8000-{id:012}"),
        start_ms: Some(start_ms),
        end_ms: Some(end_ms),
        start_date: None,
        end_date: None,
        content: format!("记录{id}"),
        tag: Some(tag),
        state: EntryState::Completed,
        updated_at: start_ms,
        deleted_at: 0,
    }
}

#[test]
fn week_range_starts_on_monday_and_crosses_year_boundaries() {
    let range = week_range("2023-01-01").unwrap();
    assert_eq!(range.days[0].date, "2022-12-26");
    assert_eq!(range.days[6].date, "2023-01-01");
    assert_eq!(range.end_ms - range.start_ms, 7 * 86_400_000);
    assert!(week_range("2023-02-29").is_err());
    assert!(week_range("２０２４-01-01").is_err());
    assert!(week_range("2024-02-29").is_ok());
}

#[test]
fn splits_a_record_across_new_year_dates() {
    let range = week_range("2024-12-30").unwrap();
    assert_eq!(range.days[1].date, "2024-12-31");
    assert_eq!(range.days[2].date, "2025-01-01");
    let entry = completed(
        1,
        range.days[1].start_ms + 23 * 60 * MINUTE_MS,
        range.days[2].start_ms + 30 * MINUTE_MS,
        Tag::Other,
    );
    let week = assemble_week(range, vec![entry]);
    assert_eq!(week.days[1].segments.len(), 1);
    assert_eq!(week.days[2].segments.len(), 1);
    assert_eq!(week.totals.other_ms, 90 * MINUTE_MS);
}

#[test]
fn splits_midnight_and_cross_week_records_and_clips_both_weeks() {
    let first_range = week_range("2024-01-01").unwrap();
    let start = first_range.days[1].start_ms + (23 * 60 + 30) * MINUTE_MS;
    let end = first_range.days[2].start_ms + 15 * MINUTE_MS;
    let overnight = completed(1, start, end, Tag::Leisure);
    let overnight_week = assemble_week(first_range, vec![overnight.clone()]);
    assert_eq!(overnight_week.days[1].segments.len(), 1);
    assert_eq!(overnight_week.days[2].segments.len(), 1);
    assert_eq!(overnight_week.totals.leisure_ms, 45 * MINUTE_MS);

    let first_range = week_range("2024-01-01").unwrap();
    let start = first_range.days[6].start_ms + 23 * 60 * MINUTE_MS;
    let next_week_start = first_range.end_ms;
    let cross_week = completed(2, start, next_week_start + 60 * MINUTE_MS, Tag::Work);
    let this_week = assemble_week(first_range, vec![cross_week.clone()]);
    assert_eq!(this_week.totals.work_ms, 60 * MINUTE_MS);
    assert_eq!(this_week.days[6].segments[0].end_ms, next_week_start);

    let next_week = assemble_week(week_range("2024-01-08").unwrap(), vec![cross_week]);
    assert_eq!(next_week.totals.work_ms, 60 * MINUTE_MS);
    assert_eq!(next_week.days[0].segments[0].start_ms, next_week_start);
}

#[test]
fn totals_keep_millisecond_precision_for_all_four_categories() {
    let range = week_range("2024-01-01").unwrap();
    let start = range.days[0].start_ms;
    let durations = [1_001, 2_002, 3_003, 4_004];
    let tags = [Tag::Work, Tag::Leisure, Tag::Sleep, Tag::Other];
    let mut cursor = start;
    let entries = durations
        .iter()
        .zip(tags)
        .enumerate()
        .map(|(index, (duration, tag))| {
            let entry = completed(index as i64 + 1, cursor, cursor + duration, tag);
            cursor += duration;
            entry
        })
        .collect();

    let week = assemble_week(range, entries);
    assert_eq!(week.totals.work_ms, durations[0]);
    assert_eq!(week.totals.leisure_ms, durations[1]);
    assert_eq!(week.totals.sleep_ms, durations[2]);
    assert_eq!(week.totals.other_ms, durations[3]);
    assert_eq!(week.totals.total_ms, durations.iter().sum::<i64>());
}

#[test]
fn database_week_query_includes_intersections_and_excludes_incomplete_entries() {
    let mut connection = db::open_memory().unwrap();
    let range = week_range("2024-01-03").unwrap();
    connection
        .execute(
            "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (?1, ?2, '跨周记录', 'sleep', 'completed')",
            [range.start_ms - 60 * MINUTE_MS, range.start_ms + 60 * MINUTE_MS],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO time_entries (start_ms, end_ms, content, state) VALUES (?1, ?2, '', 'draft')",
            [range.days[1].start_ms, range.days[1].start_ms + 30 * MINUTE_MS],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO time_entries (start_ms, content, state) VALUES (?1, '', 'running')",
            [range.days[2].start_ms],
        )
        .unwrap();
    for (start_ms, end_ms) in [
        (range.start_ms - 2 * MINUTE_MS, range.start_ms),
        (range.end_ms, range.end_ms + MINUTE_MS),
    ] {
        connection
            .execute(
                "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (?1, ?2, '边界记录', 'work', 'completed')",
                [start_ms, end_ms],
            )
            .unwrap();
    }

    let week = db::get_week(&mut connection, "2024-01-03").unwrap();
    assert_eq!(week.entries.len(), 1);
    assert_eq!(week.totals.sleep_ms, 60 * MINUTE_MS);
    assert_eq!(week.totals.total_ms, 60 * MINUTE_MS);
    assert_eq!(week.days[0].segments[0].start_ms, range.start_ms);
}

#[test]
fn all_totals_include_every_week_but_exclude_drafts() {
    let mut connection = db::open_memory().unwrap();
    for (start, end, tag) in [
        (0, 60_000, "work"),
        (604_800_000, 604_920_000, "work"),
        (900_000_000, 900_180_000, "sleep"),
    ] {
        connection.execute(
            "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (?1, ?2, '记录', ?3, 'completed')",
            rusqlite::params![start, end, tag],
        ).unwrap();
    }
    connection.execute(
        "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (1000000000, 1000360000, '', 'leisure', 'draft')",
        [],
    ).unwrap();

    let totals = db::get_all_totals(&mut connection).unwrap();
    assert_eq!(totals.work_ms, 180_000);
    assert_eq!(totals.sleep_ms, 180_000);
    assert_eq!(totals.leisure_ms, 0);
    assert_eq!(totals.other_ms, 0);
}

#[test]
fn day_totals_clip_cross_midnight_records_and_exclude_drafts() {
    let mut connection = db::open_memory().unwrap();
    let day = assemble_day("2024-01-01", Vec::new()).unwrap();
    let day_start = day.week_start_ms;
    assert_eq!(day.days.len(), 1);
    assert_eq!(day.days[0].date, "2024-01-01");
    assert_eq!(day.week_end_ms - day.week_start_ms, 86_400_000);
    assert!(assemble_day("2024-02-31", Vec::new()).is_err());

    for (start_ms, end_ms, tag) in [
        (day_start + 8 * 3_600_000, day_start + 10 * 3_600_000, "work"),
        (day_start - 3_600_000, day_start + 1_800_000, "sleep"),
        (day_start + 23 * 3_600_000, day_start + 25 * 3_600_000, "leisure"),
        (day_start - 3 * 3_600_000, day_start - 3_600_000, "other"),
    ] {
        connection
            .execute(
                "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (?1, ?2, '记录', ?3, 'completed')",
                rusqlite::params![start_ms, end_ms, tag],
            )
            .unwrap();
    }
    connection
        .execute(
            "INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (?1, ?2, '', 'work', 'draft')",
            rusqlite::params![day_start + 12 * 3_600_000, day_start + 13 * 3_600_000],
        )
        .unwrap();

    let totals = db::get_day_totals(&mut connection, "2024-01-01").unwrap();
    assert_eq!(totals.work_ms, 2 * 3_600_000);
    assert_eq!(totals.sleep_ms, 1_800_000);
    assert_eq!(totals.leisure_ms, 3_600_000);
    assert_eq!(totals.other_ms, 0);
    assert_eq!(totals.total_ms, 2 * 3_600_000 + 1_800_000 + 3_600_000);
}
