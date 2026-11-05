-- Download pipelines: direct HTTP engine vs. media pipeline (yt-dlp + FFmpeg).
ALTER TABLE downloads ADD COLUMN kind TEXT NOT NULL DEFAULT 'http'
    CHECK (kind IN ('http', 'hls', 'dash', 'video', 'audio'));
-- Media choice (JSON) and expected size live in download_metadata under 'media_choice' / 'expected_bytes'.
