use super::entries::{get_entry, read_entry, PUBLIC_ID_SQL};
use crate::{error::AppError, records::Entry};
use rusqlite::{Connection, TransactionBehavior};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn start_punch(connection: &mut Connection) -> Result<Entry, AppError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AppError::from)?;
    let start_ms = current_time_ms()?;
    let sql = format!(
        "INSERT INTO time_entries (public_id, start_ms, content, state, updated_at, deleted_at) VALUES ({PUBLIC_ID_SQL}, ?1, '', 'draft', ?1, 0)"
    );
    transaction
        .execute(&sql, [start_ms])
        .map_err(AppError::from)?;
    let entry = get_entry(&transaction, transaction.last_insert_rowid())?;
    transaction.commit().map_err(AppError::from)?;
    Ok(entry)
}

pub fn list_pending(connection: &mut Connection) -> Result<Vec<Entry>, AppError> {
    let mut statement = connection
        .prepare("SELECT id, start_ms, end_ms, content, tag, state, start_date, end_date, public_id, updated_at, deleted_at FROM time_entries WHERE state IN ('running', 'draft') AND deleted_at = 0 ORDER BY COALESCE(start_ms, end_ms) DESC, id DESC")
        .map_err(AppError::from)?;
    let entries = statement
        .query_map([], read_entry)
        .map_err(AppError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    Ok(entries)
}

fn current_time_ms() -> Result<i64, AppError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .ok_or_else(AppError::clock)
}

#[cfg(test)]
mod tests {
    use crate::{db, records::EntryState};

    #[test]
    fn every_punch_creates_a_start_only_draft() {
        let mut connection = db::open_memory().unwrap();
        let first = db::start_punch(&mut connection).unwrap();
        let second = db::start_punch(&mut connection).unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(first.state, EntryState::Draft);
        assert_eq!(first.end_ms, None);
        assert_eq!(second.end_ms, None);
        assert_eq!(db::list_pending(&mut connection).unwrap().len(), 2);
    }
}
