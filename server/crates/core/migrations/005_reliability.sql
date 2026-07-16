-- Reliability hardening for jobs and extraction materialization.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS dedupe_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS jobs_active_dedupe_uidx
    ON jobs (dedupe_key)
    WHERE dedupe_key IS NOT NULL AND status = 'queued';

ALTER TABLE import_batches ADD COLUMN IF NOT EXISTS processing_started_at TIMESTAMPTZ;

ALTER TABLE extractions ADD COLUMN IF NOT EXISTS job_id UUID;

CREATE UNIQUE INDEX IF NOT EXISTS extractions_job_uidx
    ON extractions (job_id)
    WHERE job_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS batch_applied_items (
    batch_id   TEXT NOT NULL,
    custom_id  TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (batch_id, custom_id)
);
