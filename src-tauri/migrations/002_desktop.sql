
CREATE TABLE notes (
 id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
 text TEXT NOT NULL DEFAULT '', color TEXT NOT NULL DEFAULT 'amber', revision INTEGER NOT NULL DEFAULT 0,
 is_open INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE pins (
 id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
 image_id TEXT NOT NULL REFERENCES images(id) ON DELETE RESTRICT,
 zoom REAL NOT NULL DEFAULT 1 CHECK(zoom >= 0.1 AND zoom <= 5),
 is_open INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL
);
CREATE TABLE window_states (
 object_kind TEXT NOT NULL CHECK(object_kind IN ('pin','note')), object_id TEXT NOT NULL,
 x INTEGER NOT NULL, y INTEGER NOT NULL, width INTEGER NOT NULL CHECK(width > 0),
 height INTEGER NOT NULL CHECK(height > 0), topmost INTEGER NOT NULL,
 PRIMARY KEY(object_kind, object_id)
);
CREATE INDEX notes_workspace_idx ON notes(workspace_id, updated_at DESC);
CREATE INDEX pins_workspace_idx ON pins(workspace_id, created_at DESC);
