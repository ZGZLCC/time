CREATE TABLE time_entries (
    id INTEGER PRIMARY KEY,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER,
    content TEXT NOT NULL DEFAULT '',
    tag TEXT,
    state TEXT NOT NULL,
    CHECK (state IN ('running', 'draft', 'completed')),
    CHECK (tag IS NULL OR tag IN ('work', 'leisure', 'sleep', 'other')),
    CHECK (
        (state = 'running' AND end_ms IS NULL)
        OR (state IN ('draft', 'completed') AND end_ms IS NOT NULL AND end_ms > start_ms)
    ),
    CHECK (
        state <> 'completed'
        OR (length(trim(content)) BETWEEN 1 AND 500 AND tag IS NOT NULL)
    )
) STRICT;

CREATE UNIQUE INDEX one_running_entry
    ON time_entries(state)
    WHERE state = 'running';

CREATE INDEX time_entries_state_range_idx
    ON time_entries(state, start_ms, end_ms);
