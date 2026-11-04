-- Gale schema v1. Secrets are never stored here (see credential_ref columns).

CREATE TABLE download_categories (
    id           INTEGER PRIMARY KEY,
    name         TEXT    NOT NULL UNIQUE,
    extensions   TEXT    NOT NULL DEFAULT '',          -- space-separated, lowercase, no dots
    subfolder    TEXT    NOT NULL,
    is_builtin   INTEGER NOT NULL DEFAULT 0 CHECK (is_builtin IN (0, 1)),
    sort_order   INTEGER NOT NULL DEFAULT 0
);

INSERT INTO download_categories (name, extensions, subfolder, is_builtin, sort_order) VALUES
  ('Video',     'mp4 mkv webm avi mov wmv flv m4v mpg mpeg 3gp ts', 'Video',      1, 1),
  ('Audio',     'mp3 m4a aac flac wav ogg opus wma',                 'Music',      1, 2),
  ('Documents', 'pdf doc docx xls xlsx ppt pptx odt ods txt rtf epub csv', 'Documents', 1, 3),
  ('Images',    'jpg jpeg png gif webp bmp svg tif tiff heic avif',  'Images',     1, 4),
  ('Archives',  'zip rar 7z tar gz bz2 xz zst iso',                  'Compressed', 1, 5),
  ('Software',  'exe msi msix appx appxbundle apk dmg deb rpm',      'Programs',   1, 6),
  ('Other',     '',                                                  'Other',      1, 7);

CREATE TABLE queues (
    id                    INTEGER PRIMARY KEY,
    name                  TEXT    NOT NULL UNIQUE,
    priority              INTEGER NOT NULL DEFAULT 0,
    max_concurrent        INTEGER NOT NULL DEFAULT 3 CHECK (max_concurrent BETWEEN 1 AND 32),
    max_connections       INTEGER NOT NULL DEFAULT 8 CHECK (max_connections BETWEEN 1 AND 32),
    state                 TEXT    NOT NULL DEFAULT 'stopped' CHECK (state IN ('running', 'stopped', 'paused')),
    is_default            INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO queues (name, is_default, state) VALUES ('Main queue', 1, 'running');

CREATE TABLE proxy_profiles (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    kind            TEXT NOT NULL CHECK (kind IN ('http', 'https', 'socks5')),
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL CHECK (port BETWEEN 1 AND 65535),
    username        TEXT,
    credential_ref  TEXT,          -- Windows Credential Manager target name; never a password
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE downloads (
    id               INTEGER PRIMARY KEY,
    url              TEXT    NOT NULL,
    final_url        TEXT,
    referrer         TEXT,
    file_name        TEXT    NOT NULL,
    save_dir         TEXT    NOT NULL,
    temp_path        TEXT,
    final_path       TEXT,
    mime_type        TEXT,
    total_bytes      INTEGER,                         -- NULL = unknown length
    downloaded_bytes INTEGER NOT NULL DEFAULT 0,
    status           TEXT    NOT NULL DEFAULT 'queued'
                     CHECK (status IN ('queued','probing','downloading','paused','completed','failed','cancelled','scheduled')),
    resumable        INTEGER NOT NULL DEFAULT 0 CHECK (resumable IN (0, 1)),
    connections      INTEGER NOT NULL DEFAULT 8 CHECK (connections BETWEEN 1 AND 32),
    etag             TEXT,
    last_modified    TEXT,
    sha256           TEXT,
    expected_sha256  TEXT,
    error_code       TEXT,
    error_message    TEXT,
    category_id      INTEGER REFERENCES download_categories(id) ON DELETE SET NULL,
    proxy_id         INTEGER REFERENCES proxy_profiles(id) ON DELETE SET NULL,
    source           TEXT    NOT NULL DEFAULT 'manual' CHECK (source IN ('manual','browser','batch','grabber','scheduler')),
    created_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at       TEXT,
    completed_at     TEXT,
    updated_at       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_downloads_status   ON downloads(status);
CREATE INDEX idx_downloads_category ON downloads(category_id);
CREATE INDEX idx_downloads_created  ON downloads(created_at);
CREATE INDEX idx_downloads_url      ON downloads(url);

CREATE TABLE download_segments (
    download_id  INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    idx          INTEGER NOT NULL,
    start_byte   INTEGER NOT NULL,
    end_byte     INTEGER,                              -- exclusive; NULL when length unknown
    written      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (download_id, idx),
    CHECK (end_byte IS NULL OR end_byte >= start_byte)
);

CREATE TABLE download_metadata (
    download_id  INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    key          TEXT    NOT NULL,
    value        TEXT    NOT NULL,
    PRIMARY KEY (download_id, key)
);

CREATE TABLE queue_items (
    queue_id     INTEGER NOT NULL REFERENCES queues(id) ON DELETE CASCADE,
    download_id  INTEGER NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    PRIMARY KEY (queue_id, download_id)
);
CREATE INDEX idx_queue_items_order ON queue_items(queue_id, position);

CREATE TABLE scheduler_jobs (
    id              INTEGER PRIMARY KEY,
    queue_id        INTEGER REFERENCES queues(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('start_queue', 'stop_queue')),
    run_at          TEXT NOT NULL,                     -- next UTC fire time
    recurrence      TEXT,                              -- NULL | 'daily' | 'weekdays' | 'weekly:MO,TU'
    enabled         INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    shutdown_after  INTEGER NOT NULL DEFAULT 0 CHECK (shutdown_after IN (0, 1)),
    last_run_at     TEXT
);
CREATE INDEX idx_scheduler_next ON scheduler_jobs(enabled, run_at);

CREATE TABLE browser_events (
    id           INTEGER PRIMARY KEY,
    kind         TEXT NOT NULL,                        -- intercepted | bypassed | rejected | media_detected
    url          TEXT,
    page_url     TEXT,
    detail       TEXT,                                 -- never contains cookies/headers
    download_id  INTEGER REFERENCES downloads(id) ON DELETE SET NULL,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_browser_events_created ON browser_events(created_at);

-- Append-only log of finished transfers; survives removal of the download row.
CREATE TABLE download_history (
    id            INTEGER PRIMARY KEY,
    download_id   INTEGER REFERENCES downloads(id) ON DELETE SET NULL,
    url           TEXT    NOT NULL,
    file_name     TEXT    NOT NULL,
    final_path    TEXT,
    bytes         INTEGER NOT NULL DEFAULT 0,
    outcome       TEXT    NOT NULL CHECK (outcome IN ('completed', 'failed', 'cancelled')),
    category_id   INTEGER REFERENCES download_categories(id) ON DELETE SET NULL,
    duration_ms   INTEGER,
    completed_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_history_completed ON download_history(completed_at);

CREATE TABLE application_settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,                         -- JSON-encoded
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE domain_rules (
    id         INTEGER PRIMARY KEY,
    pattern    TEXT NOT NULL UNIQUE,                   -- host or *.suffix
    action     TEXT NOT NULL CHECK (action IN ('never_intercept', 'always_intercept', 'ask')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE notifications (
    id           INTEGER PRIMARY KEY,
    level        TEXT NOT NULL CHECK (level IN ('info', 'success', 'warning', 'error')),
    title        TEXT NOT NULL,
    body         TEXT,
    download_id  INTEGER REFERENCES downloads(id) ON DELETE SET NULL,
    is_read      INTEGER NOT NULL DEFAULT 0 CHECK (is_read IN (0, 1)),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_notifications_unread ON notifications(is_read, created_at);
