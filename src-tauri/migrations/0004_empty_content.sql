-- 1.3.3 起「有起止时间和标签、只是没写内容」也算完整记录。
--
-- 原表的 CHECK 里写着 `length(trim(content)) BETWEEN 1 AND 500`，
-- 空内容的长度是 0，永远不满足，于是保存时报数据库错误。
-- SQLite 不能直接删约束，按老办法重建表：复制数据 → 换名 → 重建索引。
--
-- 约束改为「内容不超过 500 字」，允许为空；长度上限仍然守着。
CREATE TABLE time_entries_new (
    id INTEGER PRIMARY KEY,
    start_ms INTEGER,
    end_ms INTEGER,
    start_date TEXT,
    end_date TEXT,
    content TEXT NOT NULL DEFAULT '',
    tag TEXT,
    state TEXT NOT NULL,
    public_id TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0,
    CHECK (state IN ('running', 'draft', 'completed')),
    CHECK (tag IS NULL OR tag IN ('work', 'leisure', 'sleep', 'other')),
    CHECK (start_ms IS NULL OR end_ms IS NULL OR end_ms > start_ms),
    CHECK (length(trim(content)) <= 500),
    CHECK (state <> 'completed' OR (start_ms IS NOT NULL AND end_ms IS NOT NULL AND tag IS NOT NULL))
) STRICT;

INSERT INTO time_entries_new
    (id, start_ms, end_ms, start_date, end_date, content, tag, state, public_id, updated_at, deleted_at)
SELECT
    id, start_ms, end_ms, start_date, end_date, content, tag, state, public_id, updated_at, deleted_at
FROM time_entries;

DROP TABLE time_entries;
ALTER TABLE time_entries_new RENAME TO time_entries;

CREATE INDEX time_entries_state_range_idx ON time_entries(state, start_ms, end_ms);
CREATE INDEX time_entries_updated_at_idx ON time_entries(updated_at);
CREATE UNIQUE INDEX time_entries_public_id_idx ON time_entries(public_id) WHERE public_id <> '';
