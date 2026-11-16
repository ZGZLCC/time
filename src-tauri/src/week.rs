use crate::{
    error::AppError,
    records::{Entry, Tag},
};
use serde::Serialize;

const DAY_MS: i64 = 86_400_000;
const BEIJING_OFFSET_MS: i64 = 8 * 60 * 60 * 1000;

#[derive(Debug)]
pub struct WeekRange {
    pub selected_date: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub days: Vec<WeekDay>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekView {
    pub selected_date: String,
    pub week_start_ms: i64,
    pub week_end_ms: i64,
    pub entries: Vec<Entry>,
    pub days: Vec<WeekDay>,
    pub totals: WeekTotals,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekDay {
    pub date: String,
    pub start_ms: i64,
    pub segments: Vec<WeekSegment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekSegment {
    pub entry_id: i64,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekTotals {
    pub work_ms: i64,
    pub leisure_ms: i64,
    pub sleep_ms: i64,
    pub other_ms: i64,
    pub total_ms: i64,
}

pub fn week_range(date: &str) -> Result<WeekRange, AppError> {
    let (year, month, day) = parse_date(date)?;
    let selected_day = days_from_civil(year, month, day);
    let monday = selected_day - (selected_day + 3).rem_euclid(7);
    let start_ms = monday * DAY_MS - BEIJING_OFFSET_MS;
    let days = (0..7)
        .map(|offset| {
            let day_number = monday + offset;
            WeekDay {
                date: format_date(day_number),
                start_ms: start_ms + offset * DAY_MS,
                segments: Vec::new(),
            }
        })
        .collect();
    Ok(WeekRange {
        selected_date: date.to_owned(),
        start_ms,
        end_ms: start_ms + 7 * DAY_MS,
        days,
    })
}

pub fn assemble_week(range: WeekRange, entries: Vec<Entry>) -> WeekView {
    let mut totals = WeekTotals::default();
    let mut days = range.days;
    for entry in &entries {
        let (Some(start_ms), Some(end_ms), Some(tag)) = (entry.start_ms, entry.end_ms, entry.tag)
        else {
            continue;
        };
        if entry.state != crate::records::EntryState::Completed {
            continue;
        }
        let start_ms = start_ms.max(range.start_ms);
        let end_ms = end_ms.min(range.end_ms);
        if end_ms <= start_ms {
            continue;
        }
        totals.add(tag, end_ms - start_ms);
        for day in &mut days {
            let clipped_start = start_ms.max(day.start_ms);
            let clipped_end = end_ms.min(day.start_ms + DAY_MS);
            if clipped_start < clipped_end {
                day.segments.push(WeekSegment {
                    entry_id: entry.id,
                    start_ms: clipped_start,
                    end_ms: clipped_end,
                });
            }
        }
    }
    WeekView {
        selected_date: range.selected_date,
        week_start_ms: range.start_ms,
        week_end_ms: range.end_ms,
        entries,
        days,
        totals,
    }
}

impl WeekTotals {
    pub(crate) fn add(&mut self, tag: Tag, duration_ms: i64) {
        match tag {
            Tag::Work => self.work_ms += duration_ms,
            Tag::Leisure => self.leisure_ms += duration_ms,
            Tag::Sleep => self.sleep_ms += duration_ms,
            Tag::Other => self.other_ms += duration_ms,
        }
        self.total_ms += duration_ms;
    }
}

fn parse_date(value: &str) -> Result<(i64, i64, i64), AppError> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return Err(AppError::validation("日期格式无效，请使用 YYYY-MM-DD"));
    }
    let year = value[..4].parse::<i64>().ok();
    let month = value[5..7].parse::<i64>().ok();
    let day = value[8..10].parse::<i64>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return Err(AppError::validation("日期格式无效，请使用 YYYY-MM-DD"));
    };
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    };
    if year < 1 || day < 1 || day > days_in_month {
        return Err(AppError::validation("日期无效"));
    }
    Ok((year, month, day))
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

// Convert proleptic Gregorian dates to days since the Unix epoch without local-time rules.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn format_date(days: i64) -> String {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}")
}
