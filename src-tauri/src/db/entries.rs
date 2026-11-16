use crate::{
    error::AppError,
    records::{validate, Entry, EntryState, SaveEntryInput, Tag, ValidatedEntry},
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

pub fn save_entry(connection: &mut Connection, input: SaveEntryInput) -> Result<Entry, AppError> {
    let entry = validate(input)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AppError::from)?;

    if entry.state == EntryState::Completed {
        if let Some(id) = find_overlap(&transaction, &entry)? {
            return Err(AppError::overlap(id));
        }
    }

    let now = now_ms();
    let id = if let Some(id) = entry.id {
        let changed = transaction
            .execute(
                "UPDATE time_entries SET start_ms = ?1, end_ms = ?2, start_date = ?3, end_date = ?4, content = ?5, tag = ?6, state = ?7, updated_at = ?8 WHERE id = ?9",
                params![entry.start_ms, entry.end_ms, entry.start_date, entry.end_date, entry.content, entry.tag.map(Tag::as_str), entry.state.as_str(), now, id],
            )
            .map_err(AppError::from)?;
        if changed == 0 {
            return Err(AppError::not_found());
        }
        id
    } else {
        let sql = format!(
            "INSERT INTO time_entries (public_id, start_ms, end_ms, start_date, end_date, content, tag, state, updated_at, deleted_at) VALUES ({PUBLIC_ID_SQL}, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)"
        );
        transaction
            .execute(
                &sql,
                params![entry.start_ms, entry.end_ms, entry.start_date, entry.end_date, entry.content, entry.tag.map(Tag::as_str), entry.state.as_str(), now],
            )
            .map_err(AppError::from)?;
        transaction.last_insert_rowid()
    };

    let saved = get_entry(&transaction, id)?;
    transaction.commit().map_err(AppError::from)?;
    Ok(saved)
}

pub fn list_entries(connection: &mut Connection) -> Result<Vec<Entry>, AppError> {
    let mut statement = connection
        .prepare("SELECT id, start_ms, end_ms, content, tag, state, start_date, end_date, public_id, updated_at, deleted_at FROM time_entries WHERE deleted_at = 0 ORDER BY COALESCE(start_ms, end_ms) DESC, id DESC")
        .map_err(AppError::from)?;
    let entries = statement
        .query_map([], read_entry)
        .map_err(AppError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    Ok(entries)
}

/// 软删除：只打删除标记，不删行，这样删除动作能同步到其他端。
pub fn delete_entry(connection: &mut Connection, id: i64) -> Result<(), AppError> {
    if id <= 0 {
        return Err(AppError::validation("记录编号无效"));
    }
    let changed = connection
        .execute(
            "UPDATE time_entries SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at = 0",
            params![now_ms(), id],
        )
        .map_err(AppError::from)?;
    if changed == 0 {
        return Err(AppError::not_found());
    }
    Ok(())
}

/// 当前 Unix 毫秒。同步用，也用于 updated_at / deleted_at。
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0)
}

/// 生成 v4 UUID 的 SQL 片段。用 SQLite 自带的 randomblob/hex，
/// 与迁移 0003 里的写法一致，不额外引入 uuid crate。
pub(super) const PUBLIC_ID_SQL: &str = "lower(
      substr(hex(randomblob(4)), 1, 8) || '-' ||
      substr(hex(randomblob(2)), 1, 4) || '-4' ||
      substr(hex(randomblob(2)), 2, 3) || '-' ||
      substr('89ab', abs(random()) % 4 + 1, 1) ||
      substr(hex(randomblob(2)), 2, 3) || '-' ||
      substr(hex(randomblob(6)), 1, 12)
    )";

/// 同步用：读取全部记录，含已软删除的（墓碑也要参与合并，否则删除会复活）。
pub fn list_all_for_sync(connection: &mut Connection) -> Result<Vec<Entry>, AppError> {
    let mut statement = connection
        .prepare("SELECT id, start_ms, end_ms, content, tag, state, start_date, end_date, public_id, updated_at, deleted_at FROM time_entries ORDER BY id")
        .map_err(AppError::from)?;
    let entries = statement
        .query_map([], read_entry)
        .map_err(AppError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    Ok(entries)
}

/// 云端记录：与 Worker 的字段一一对应，用 public_id 与本地对应。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudEntry {
    pub id: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub content: String,
    pub tag: Option<String>,
    pub state: String,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

impl From<&Entry> for CloudEntry {
    fn from(entry: &Entry) -> Self {
        Self {
            id: entry.public_id.clone(),
            start_ms: entry.start_ms,
            end_ms: entry.end_ms,
            start_date: entry.start_date.clone(),
            end_date: entry.end_date.clone(),
            content: entry.content.clone(),
            tag: entry.tag.map(|tag| tag.as_str().to_owned()),
            // 云端的枚举与本地不同：云端的 running 对应本地的 draft
            state: match entry.state {
                EntryState::Completed => "completed".to_owned(),
                _ => "draft".to_owned(),
            },
            updated_at: entry.updated_at,
            deleted_at: if entry.deleted_at == 0 { None } else { Some(entry.deleted_at) },
        }
    }
}

/// 合并结果：写回本地的条数，以及需要推给云端的更晚版本。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOutcome {
    pub applied: usize,
    pub pending: Vec<CloudEntry>,
}

/// 按 public_id 合并云端快照到本地，规则与网页版一致：updated_at 更晚者胜出。
///
/// 本地没有的（换机器、重装后拉回）按云端内容插入；
/// 本地更新的（云端快照过期）不覆盖，并挑出来交给调用方推回云端。
pub fn merge_cloud_entries(
    connection: &mut Connection,
    incoming: &[CloudEntry],
) -> Result<MergeOutcome, AppError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AppError::from)?;
    let mut applied = 0usize;

    for entry in incoming {
        if entry.id.trim().is_empty() {
            continue;
        }
        let tag = entry.tag.as_deref().and_then(Tag::from_db);
        let state = match entry.state.as_str() {
            "completed" => EntryState::Completed,
            _ => EntryState::Draft,
        };
        let deleted_at = entry.deleted_at.unwrap_or(0);
        // 与 validate 保持一致：已完成记录必须有起止时间和标签。
        // 内容可以为空（1.3.3 起），所以这里和 mirror_cloud_entries 一样不检查内容——
        // 否则网页版建的「有起止时间和标签、只是没写内容」的记录会被静默丢掉。
        if state == EntryState::Completed
            && (entry.start_ms.is_none() || entry.end_ms.is_none() || tag.is_none())
        {
            continue;
        }
        let existing: Option<(i64, i64)> = transaction
            .query_row(
                "SELECT id, updated_at FROM time_entries WHERE public_id = ?1",
                [&entry.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(AppError::from)?;

        match existing {
            Some((id, local_updated_at)) => {
                if entry.updated_at > local_updated_at {
                    transaction
                        .execute(
                            "UPDATE time_entries SET start_ms = ?1, end_ms = ?2, start_date = ?3, end_date = ?4, content = ?5, tag = ?6, state = ?7, updated_at = ?8, deleted_at = ?9 WHERE id = ?10",
                            params![
                                entry.start_ms,
                                entry.end_ms,
                                entry.start_date,
                                entry.end_date,
                                entry.content,
                                tag.map(Tag::as_str),
                                state.as_str(),
                                entry.updated_at,
                                deleted_at,
                                id
                            ],
                        )
                        .map_err(AppError::from)?;
                    applied += 1;
                }
            }
            None => {
                transaction
                    .execute(
                        "INSERT INTO time_entries (public_id, start_ms, end_ms, start_date, end_date, content, tag, state, updated_at, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            entry.id,
                            entry.start_ms,
                            entry.end_ms,
                            entry.start_date,
                            entry.end_date,
                            entry.content,
                            tag.map(Tag::as_str),
                            state.as_str(),
                            entry.updated_at,
                            deleted_at
                        ],
                    )
                    .map_err(AppError::from)?;
                applied += 1;
            }
        }
    }

    // 云端快照里没有的、或本地版本更晚的，都要交回给调用方推上去
    let incoming_updated: std::collections::HashMap<&str, i64> = incoming
        .iter()
        .map(|entry| (entry.id.as_str(), entry.updated_at))
        .collect();
    let mut pending: Vec<CloudEntry> = Vec::new();
    let mut statement = transaction
        .prepare("SELECT id, start_ms, end_ms, content, tag, state, start_date, end_date, public_id, updated_at, deleted_at FROM time_entries")
        .map_err(AppError::from)?;
    let locals = statement
        .query_map([], read_entry)
        .map_err(AppError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    drop(statement);
    for local in &locals {
        let newer = incoming_updated
            .get(local.public_id.as_str())
            .is_none_or(|remote| local.updated_at > *remote);
        if newer && !local.public_id.is_empty() {
            pending.push(CloudEntry::from(local));
        }
    }

    transaction.commit().map_err(AppError::from)?;
    Ok(MergeOutcome { applied, pending })
}

/// 清空全部记录：只打删除标记，不删行。
///
/// 保留墓碑是有意的：其他设备同步过去才知道这些记录被删了，
/// 否则它们会从云端重新拉回来。想彻底清空请重新建同步码或直接删库文件。
pub fn clear_entries(connection: &mut Connection) -> Result<usize, AppError> {
    let now = now_ms();
    let changed = connection
        .execute(
            "UPDATE time_entries SET deleted_at = ?1, updated_at = ?1 WHERE deleted_at = 0",
            params![now],
        )
        .map_err(AppError::from)?;
    Ok(changed)
}

/// 用给定的云端快照**整体覆盖**本地数据，返回写入条数。
///
/// 与 `merge_cloud_entries` 的区别：那个是逐条「后写胜出」的合并，
/// 这个是让本地变成快照的镜像——快照里没有的记录一律软删除。
/// 软删除会留下墓碑，随后被推回云端，于是删除也会传到其他设备，
/// 这正是「上传时用本地覆盖云端」需要的效果。
pub fn mirror_cloud_entries(
    connection: &mut Connection,
    incoming: Vec<CloudEntry>,
) -> Result<i64, AppError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AppError::from)?;

    // 先全部打上删除标记，再把快照里的记录写回去（写回时会把 deleted_at 清成 0）。
    // ponytail: 一次性全标再逐条写回，记录量在几万条以内都很快；
    // 真到几十万条再改成按 public_id 差集更新。
    let now = now_ms();
    transaction
        .execute(
            "UPDATE time_entries SET deleted_at = ?1, updated_at = ?1 WHERE deleted_at = 0",
            params![now],
        )
        .map_err(AppError::from)?;

    let mut applied = 0i64;
    for entry in incoming {
        // 与合并路径同一套校验：空编号和不合法的已完成记录一律跳过
        if entry.id.is_empty() {
            continue;
        }
        let tag = entry.tag.as_deref().and_then(Tag::from_db);
        let state = match entry.state.as_str() {
            "completed" => EntryState::Completed,
            _ => EntryState::Draft,
        };
        let deleted_at = entry.deleted_at.unwrap_or(0).max(0);
        let ok = if state == EntryState::Completed {
            entry.start_ms.is_some() && entry.end_ms.is_some() && tag.is_some()
        } else {
            entry.start_ms.is_some() || entry.end_ms.is_some()
        };
        if !ok {
            continue;
        }

        let existing: Option<i64> = transaction
            .query_row(
                "SELECT id FROM time_entries WHERE public_id = ?1",
                params![entry.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::from)?;

        match existing {
            Some(id) => {
                transaction
                    .execute(
                        "UPDATE time_entries SET start_ms = ?1, end_ms = ?2, start_date = ?3, end_date = ?4, content = ?5, tag = ?6, state = ?7, updated_at = ?8, deleted_at = ?9 WHERE id = ?10",
                        params![
                            entry.start_ms,
                            entry.end_ms,
                            entry.start_date,
                            entry.end_date,
                            entry.content,
                            tag.map(Tag::as_str),
                            state.as_str(),
                            entry.updated_at,
                            deleted_at,
                            id
                        ],
                    )
                    .map_err(AppError::from)?;
            }
            None => {
                transaction
                    .execute(
                        "INSERT INTO time_entries (public_id, start_ms, end_ms, start_date, end_date, content, tag, state, updated_at, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            entry.id,
                            entry.start_ms,
                            entry.end_ms,
                            entry.start_date,
                            entry.end_date,
                            entry.content,
                            tag.map(Tag::as_str),
                            state.as_str(),
                            entry.updated_at,
                            deleted_at
                        ],
                    )
                    .map_err(AppError::from)?;
            }
        }
        applied += 1;
    }

    transaction.commit().map_err(AppError::from)?;
    Ok(applied)
}

/// 删除墓碑的保留期，必须与网页版 `src/core/sync.ts` 和 Worker 的
/// `worker/src/entries.ts` 保持一致，否则会出现「一端清了一端没清」，
/// 让别的设备把已删记录推回来。
pub const TOMBSTONE_TTL_MS: i64 = 180 * 86_400_000;

/// 真正删除超过保留期的墓碑，返回删除条数。
///
/// 以前只做「过滤返回值」，库里一条都没删，墓碑会无限累积。
/// 保留期内的必须留着：离线很久的设备要靠它知道自己删过什么。
pub fn purge_tombstones(connection: &mut Connection, now: i64) -> Result<usize, AppError> {
    let cutoff = now - TOMBSTONE_TTL_MS;
    let removed = connection
        .execute(
            "DELETE FROM time_entries WHERE deleted_at <> 0 AND deleted_at < ?1",
            params![cutoff],
        )
        .map_err(AppError::from)?;
    Ok(removed)
}

fn find_overlap(
    transaction: &Transaction<'_>,
    entry: &ValidatedEntry,
) -> Result<Option<i64>, AppError> {
    transaction
        .query_row(
            "SELECT id FROM time_entries WHERE state = 'completed' AND deleted_at = 0 AND start_ms < ?1 AND end_ms > ?2 AND (?3 IS NULL OR id <> ?3) ORDER BY start_ms LIMIT 1",
            params![entry.end_ms, entry.start_ms, entry.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)
}

pub(super) fn get_entry(connection: &Connection, id: i64) -> Result<Entry, AppError> {
    connection
        .query_row(
            "SELECT id, start_ms, end_ms, content, tag, state, start_date, end_date, public_id, updated_at, deleted_at FROM time_entries WHERE id = ?1",
            [id],
            read_entry,
        )
        .map_err(AppError::from)
}

pub(super) fn read_entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let raw_tag: Option<String> = row.get(4)?;
    let raw_state: String = row.get(5)?;
    Ok(Entry {
        id: row.get(0)?,
        start_ms: row.get(1)?,
        end_ms: row.get(2)?,
        start_date: row.get(6)?,
        end_date: row.get(7)?,
        content: row.get(3)?,
        public_id: row.get(8)?,
        updated_at: row.get(9)?,
        deleted_at: row.get(10)?,
        tag: raw_tag
            .as_deref()
            .map(|tag| parse_db_value(tag, 4, "tag", Tag::from_db))
            .transpose()?,
        state: parse_db_value(&raw_state, 5, "state", EntryState::from_db)?,
    })
}

fn parse_db_value<T>(
    value: &str,
    index: usize,
    column: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> rusqlite::Result<T> {
    parse(value).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(index, column.to_owned(), rusqlite::types::Type::Text)
    })
}
