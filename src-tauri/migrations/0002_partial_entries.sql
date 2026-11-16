CREATE TABLE time_entries_new (
    id INTEGER PRIMARY KEY,
    start_ms INTEGER,
    end_ms INTEGER,
    start_date TEXT,
    end_date TEXT,
    content TEXT NOT NULL DEFAULT '',
    tag TEXT,
    state TEXT NOT NULL,
    CHECK (state IN ('running', 'draft', 'completed')),
    CHECK (tag IS NULL OR tag IN ('work', 'leisure', 'sleep', 'other')),
    CHECK (start_ms IS NULL OR end_ms IS NULL OR end_ms > start_ms),
    CHECK (state <> 'completed' OR (start_ms IS NOT NULL AND end_ms IS NOT NULL AND length(trim(content)) BETWEEN 1 AND 500 AND tag IS NOT NULL))
) STRICT;

INSERT INTO time_entries_new (id, start_ms, end_ms, content, tag, state)
SELECT id, start_ms, end_ms, content, tag, CASE WHEN state = 'running' THEN 'draft' ELSE state END
FROM time_entries;

DROP TABLE time_entries;
ALTER TABLE time_entries_new RENAME TO time_entries;

CREATE INDEX time_entries_state_range_idx ON time_entries(state, start_ms, end_ms);
