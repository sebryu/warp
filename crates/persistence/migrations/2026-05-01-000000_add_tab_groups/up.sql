-- Tab groups (Chrome-like): a window-local container of contiguous tabs.
-- Identity is a UUID stored as TEXT, NOT the integer PK, because `tabs.id`
-- and `tab_groups.id` are both regenerated on every save
-- (see app/src/persistence/sqlite.rs save_app_state).
CREATE TABLE tab_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    uuid TEXT NOT NULL,
    window_id INTEGER NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL,
    is_collapsed INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (window_id) REFERENCES windows(id) ON DELETE CASCADE,
    UNIQUE (window_id, uuid)
);

-- Per-tab pointer to the tab's group, by stable UUID. Nullable: ungrouped tabs
-- have NULL. The integer FK from tabs.id to anything is unsafe because tab IDs
-- are regenerated on every save; the UUID join is stable across save cycles.
ALTER TABLE tabs ADD COLUMN group_uuid TEXT;
