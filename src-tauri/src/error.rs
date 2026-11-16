use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Validation,
    Overlap,
    NotFound,
    Database,
    Migration,
    Clock,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflicting_entry_id: Option<i64>,
}

impl AppError {
    pub fn validation(message: &'static str) -> Self {
        Self::new(ErrorCode::Validation, message)
    }

    pub fn overlap(id: i64) -> Self {
        Self {
            code: ErrorCode::Overlap,
            message: "时间与已有记录重叠",
            conflicting_entry_id: Some(id),
        }
    }

    pub fn not_found() -> Self {
        Self::new(ErrorCode::NotFound, "记录不存在或已被删除")
    }

    pub fn database() -> Self {
        Self::new(
            ErrorCode::Database,
            "本地数据库操作失败，请检查应用数据目录权限后重试",
        )
    }

    pub fn database_missing() -> Self {
        Self::new(
            ErrorCode::Database,
            "数据库文件不可用，请检查设置的数据库位置后重试",
        )
    }

    pub fn migration() -> Self {
        Self::new(
            ErrorCode::Migration,
            "本地数据库迁移失败，原有数据已保留；请关闭应用后重试",
        )
    }

    pub fn clock() -> Self {
        Self::new(
            ErrorCode::Clock,
            "系统时间早于开始时间，请校准系统时间后重试",
        )
    }

    fn new(code: ErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message,
            conflicting_entry_id: None,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        Self::database()
    }
}
