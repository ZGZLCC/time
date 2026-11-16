use super::entries::read_entry;
use crate::{
    day::assemble_day,
    error::AppError,
    records::EntryState,
    week::{assemble_week, week_range, WeekTotals, WeekView},
};
use rusqlite::{params, Connection};

const ENTRY_COLUMNS: &str = "SELECT id, start_ms, end_ms, content, tag, state, start_date, end_date, public_id, updated_at, deleted_at FROM time_entries WHERE state = 'completed' AND deleted_at = 0 AND start_ms < ?1 AND end_ms > ?2 ORDER BY start_ms, id";

fn intersecting_entries(
    connection: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<crate::records::Entry>, AppError> {
    let mut statement = connection.prepare(ENTRY_COLUMNS).map_err(AppError::from)?;
    let rows = statement
        .query_map(params![end_ms, start_ms], read_entry)
        .map_err(AppError::from)?;
    let entries = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    Ok(entries)
}

pub fn get_week(connection: &mut Connection, date: &str) -> Result<WeekView, AppError> {
    let range = week_range(date)?;
    let entries = intersecting_entries(connection, range.start_ms, range.end_ms)?;
    Ok(assemble_week(range, entries))
}

pub fn get_day_totals(connection: &mut Connection, date: &str) -> Result<WeekTotals, AppError> {
    let day = assemble_day(date, Vec::new())?;
    let entries = intersecting_entries(connection, day.week_start_ms, day.week_end_ms)?;
    Ok(assemble_day(date, entries)?.totals)
}

pub fn get_all_totals(connection: &mut Connection) -> Result<WeekTotals, AppError> {
    let mut totals = WeekTotals::default();
    for entry in super::entries::list_entries(connection)? {
        if entry.state == EntryState::Completed {
            if let (Some(start), Some(end), Some(tag)) = (entry.start_ms, entry.end_ms, entry.tag) {
                totals.add(tag, end - start);
            }
        }
    }
    Ok(totals)
}
