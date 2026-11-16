use crate::{
    error::AppError,
    records::Entry,
    week::{assemble_week, week_range, WeekView},
};

const DAY_MS: i64 = 86_400_000;

/// 复用周区间裁剪规则得到单日 `[00:00, 24:00)` 视图，供“本日已记录”统计使用。
pub fn assemble_day(date: &str, entries: Vec<Entry>) -> Result<WeekView, AppError> {
    let mut range = week_range(date)?;
    let offset = range
        .days
        .iter()
        .position(|day| day.date == date)
        .unwrap_or(0);
    range.start_ms += offset as i64 * DAY_MS;
    range.end_ms = range.start_ms + DAY_MS;

    let day = range.days.remove(offset);
    range.days = vec![day];
    Ok(assemble_week(range, entries))
}
