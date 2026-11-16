-- 0003：为云同步补三列。
--
-- public_id   跨端稳定标识。Web 版用 UUID 作记录 id，桌面原本是自增整数，
--             加上这一列后两端才能对上同一条记录；本地主键仍是整数，不动现有查询。
-- updated_at  修改时间，云同步按它做「后写胜出」。
-- deleted_at  删除标记（0 表示未删除）。改用软删除，删除动作才能同步到其他端。
--
-- 这三列都有 DEFAULT，STRICT 表允许，已有行会自动取默认值。

ALTER TABLE time_entries ADD COLUMN public_id TEXT NOT NULL DEFAULT '';
ALTER TABLE time_entries ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE time_entries ADD COLUMN deleted_at INTEGER NOT NULL DEFAULT 0;

-- 回填 public_id：SQLite 自带 randomblob/hex，无需扩展即可生成 v4 UUID。
-- updated_at 取记录自身的时间，保证老记录之间的先后顺序与显示顺序一致。
UPDATE time_entries
SET public_id = lower(
      substr(hex(randomblob(4)), 1, 8) || '-' ||
      substr(hex(randomblob(2)), 1, 4) || '-4' ||
      substr(hex(randomblob(2)), 2, 3) || '-' ||
      substr('89ab', abs(random()) % 4 + 1, 1) ||
      substr(hex(randomblob(2)), 2, 3) || '-' ||
      substr(hex(randomblob(6)), 1, 12)
    ),
    updated_at = COALESCE(start_ms, end_ms, 0) * 1000 + id,
    deleted_at = 0
WHERE public_id = '';

-- 只对非空 public_id 建唯一索引。
-- 写成全量唯一索引会让「不提供 public_id 的 INSERT」在第二条就撞唯一约束
-- （默认值都是空串），等于把旧代码和外部写入全堵死。
CREATE UNIQUE INDEX time_entries_public_id_idx
    ON time_entries(public_id)
    WHERE public_id <> '';
CREATE INDEX time_entries_updated_idx ON time_entries(updated_at);
