use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Work,
    Leisure,
    Sleep,
    Other,
}

impl Tag {
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "work" => Some(Self::Work),
            "leisure" => Some(Self::Leisure),
            "sleep" => Some(Self::Sleep),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Leisure => "leisure",
            Self::Sleep => "sleep",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    Running,
    Draft,
    Completed,
}

impl EntryState {
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "draft" => Some(Self::Draft),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Draft => "draft",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    /// 跨端稳定标识（UUID）。本地主键仍是 `id`，同步用这一个。
    pub public_id: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub content: String,
    pub tag: Option<Tag>,
    pub state: EntryState,
    /// 修改时间（毫秒）。云同步据此做「后写胜出」。
    pub updated_at: i64,
    /// 删除时间，0 表示未删除。用软删除，删除动作才能同步到其他端。
    pub deleted_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEntryInput {
    pub id: Option<i64>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub content: String,
    pub tag: Option<String>,
}

#[derive(Debug)]
pub struct ValidatedEntry {
    pub id: Option<i64>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub content: String,
    pub tag: Option<Tag>,
    pub state: EntryState,
}

pub fn validate(input: SaveEntryInput) -> Result<ValidatedEntry, AppError> {
    if input.id.is_some_and(|id| id <= 0) {
        return Err(AppError::validation("记录编号无效"));
    }
    let content = input.content.trim().to_owned();
    let length = content.chars().count();
    if length > 500 {
        return Err(AppError::validation("事情内容不得超过 500 个字符"));
    }
    if let (Some(start), Some(end)) = (input.start_ms, input.end_ms) {
        if end <= start {
            return Err(AppError::validation("结束时间必须晚于开始时间"));
        }
    }
    for date in [&input.start_date, &input.end_date].into_iter().flatten() {
        crate::week::week_range(date)?;
    }
    let tag = match input.tag.as_deref() {
        Some(value) => {
            Some(Tag::from_db(value).ok_or_else(|| AppError::validation("请选择有效标签"))?)
        }
        None => None,
    };
    if input.start_ms.is_none()
        && input.end_ms.is_none()
        && input.start_date.is_none()
        && input.end_date.is_none()
        && content.is_empty()
        && tag.is_none()
    {
        return Err(AppError::validation("请至少填写一项记录内容"));
    }
    // 内容可以是空的：有起止时间和标签就算已是完整记录，
    // 只是「没写做了什么」而已，不该一直挂在待补全里。
    let state = if input.start_ms.is_some() && input.end_ms.is_some() && tag.is_some() {
        EntryState::Completed
    } else {
        EntryState::Draft
    };

    Ok(ValidatedEntry {
        id: input.id,
        start_ms: input.start_ms,
        end_ms: input.end_ms,
        start_date: input.start_date,
        end_date: input.end_date,
        content,
        tag,
        state,
    })
}

#[cfg(test)]
mod tests {
    use super::{validate, SaveEntryInput};
    use crate::error::ErrorCode;

    fn input(content: &str) -> SaveEntryInput {
        SaveEntryInput {
            id: None,
            start_ms: Some(1_000),
            end_ms: Some(2_000),
            start_date: None,
            end_date: None,
            content: content.to_owned(),
            tag: Some("work".to_owned()),
        }
    }

    #[test]
    fn trims_unicode_whitespace_and_enforces_content_length() {
        let valid = validate(input("\u{3000}会议\u{3000}")).unwrap();
        assert_eq!(valid.content, "会议");

        let too_long = validate(input(&"界".repeat(501))).unwrap_err();
        assert!(matches!(too_long.code, ErrorCode::Validation));
    }

    #[test]
    fn rejects_invalid_interval_and_tag() {
        let mut invalid_interval = input("休息");
        invalid_interval.end_ms = invalid_interval.start_ms;
        assert!(validate(invalid_interval).is_err());

        let mut invalid_tag = input("休息");
        invalid_tag.tag = Some("unknown".to_owned());
        assert!(validate(invalid_tag).is_err());
    }

    #[test]
    fn incomplete_entry_becomes_draft() {
        let mut draft = input("");
        draft.end_ms = None;
        draft.tag = None;
        assert_eq!(validate(draft).unwrap().state, super::EntryState::Draft);
    }

    #[test]
    fn entry_without_content_is_still_completed() {
        // 有起止时间和标签就是完整记录，只是没写做了什么，不该挂在待补全里
        assert_eq!(
            validate(input("")).unwrap().state,
            super::EntryState::Completed
        );
        // 只有空白字符同样算没写
        assert_eq!(
            validate(input("   ")).unwrap().state,
            super::EntryState::Completed
        );
        // 缺标签仍然算草稿
        let mut no_tag = input("");
        no_tag.tag = None;
        assert_eq!(validate(no_tag).unwrap().state, super::EntryState::Draft);
    }
}
