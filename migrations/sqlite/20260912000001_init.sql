-- Managed bucket profiles
CREATE TABLE IF NOT EXISTS buckets (
    id TEXT PRIMARY KEY,
    profile_name TEXT NOT NULL UNIQUE,
    bucket_name TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Cached objects & prefixes
CREATE TABLE IF NOT EXISTS objects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_profile TEXT NOT NULL,
    object_key TEXT NOT NULL,
    parent_prefix TEXT NOT NULL,
    is_directory INTEGER NOT NULL DEFAULT 0 CHECK (is_directory IN (0, 1)),
    size_bytes INTEGER NOT NULL DEFAULT 0,
    etag TEXT,
    content_type TEXT,
    last_modified TEXT NOT NULL,
    synced_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(bucket_profile, object_key)
);

CREATE INDEX IF NOT EXISTS idx_objects_lookup 
ON objects(bucket_profile, parent_prefix, is_directory);

-- Prefix sync timestamp for CacheFreshness TTL
CREATE TABLE IF NOT EXISTS prefix_sync_status (
    bucket_profile TEXT NOT NULL,
    prefix TEXT NOT NULL,
    last_synced_at TEXT NOT NULL,
    PRIMARY KEY (bucket_profile, prefix)
);

-- Admin authentication sessions
CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at TEXT NOT NULL
);

-- Active and resumable multipart upload sessions
CREATE TABLE IF NOT EXISTS multipart_sessions (
    upload_id TEXT PRIMARY KEY,
    bucket_profile TEXT NOT NULL,
    object_key TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    part_size INTEGER NOT NULL,
    total_parts INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_activity_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
