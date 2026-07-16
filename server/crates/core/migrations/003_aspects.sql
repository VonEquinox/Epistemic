-- Multi-aspect DNA: fixed analysis layers + per-aspect neighbor dimensions.

ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_problem';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_contributions';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_methods';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_theory';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_datasets';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_findings';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_limitations';
ALTER TYPE neighbor_dimension ADD VALUE IF NOT EXISTS 'aspect_positioning';

CREATE TABLE IF NOT EXISTS paper_aspects (
    work_id         UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    aspect          TEXT NOT NULL,
    summary         TEXT NOT NULL DEFAULT '',
    bullets         JSONB NOT NULL DEFAULT '[]'::jsonb,
    source_text     TEXT NOT NULL DEFAULT '',
    page            INT NOT NULL DEFAULT 0,
    model           TEXT,
    prompt_version  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (work_id, aspect)
);

CREATE INDEX IF NOT EXISTS paper_aspects_aspect_idx ON paper_aspects (aspect);
