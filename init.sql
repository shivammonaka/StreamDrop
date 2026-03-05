CREATE TABLE videos (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug          TEXT UNIQUE NOT NULL,
    status        TEXT NOT NULL DEFAULT 'Pending',
    original_path TEXT,
    hls_path      TEXT,
    size_bytes    BIGINT,
    mime_type     TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);