-- Epistemic schema v0.1 — M0/M1 foundation
-- See docs/DEV.md §4

CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "vector";

-- ─── Enums ───────────────────────────────────────────────────────────────────

CREATE TYPE user_role AS ENUM ('admin', 'member');
CREATE TYPE version_kind AS ENUM ('arxiv', 'conference', 'journal', 'preprint', 'other');
CREATE TYPE source_layer AS ENUM ('public_fact', 'team_record', 'ai_candidate');
CREATE TYPE review_status AS ENUM ('unreviewed', 'confirmed', 'disputed', 'rejected');
CREATE TYPE relation_type AS ENUM (
    'cites',
    'version_of',
    'uses_method_from',
    'improves_on',
    'alternative_to',
    'uses_dataset_from',
    'compares_against',
    'reproduces',
    'fails_to_reproduce',
    'supports_claim',
    'contradicts_claim',
    'prerequisite_for'
);
CREATE TYPE entity_kind AS ENUM ('work', 'claim', 'method', 'dataset', 'version', 'person');
CREATE TYPE member_role AS ENUM ('source', 'target', 'input', 'output');
CREATE TYPE review_verdict AS ENUM ('agree', 'disagree');
CREATE TYPE subject_kind AS ENUM ('relation', 'claim_judgment');
CREATE TYPE claim_verdict AS ENUM (
    'supported',
    'partially_supported',
    'contradicted',
    'not_reproduced',
    'concern',
    'unclear'
);
CREATE TYPE reading_level AS ENUM ('unread', 'skimmed', 'read', 'reproduced', 'needs_review');
CREATE TYPE annotation_kind AS ENUM ('note', 'conjecture', 'question');
CREATE TYPE visibility AS ENUM ('private', 'team');
CREATE TYPE job_status AS ENUM ('queued', 'running', 'done', 'failed');
CREATE TYPE neighbor_dimension AS ENUM ('citation_coupling', 'method_lineage', 'topic');
CREATE TYPE import_status AS ENUM ('preview', 'confirmed', 'processing', 'done', 'failed');

-- ─── Accounts & org ──────────────────────────────────────────────────────────

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role          user_role NOT NULL DEFAULT 'member',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE invites (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email      TEXT NOT NULL,
    token      TEXT NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE projects (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ─── Papers ──────────────────────────────────────────────────────────────────

CREATE TABLE works (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title_norm         TEXT NOT NULL,
    primary_version_id UUID,          -- filled after first version insert
    created_by         UUID REFERENCES users(id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX works_title_norm_idx ON works (title_norm);

CREATE TABLE versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id         UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    kind            version_kind NOT NULL DEFAULT 'other',
    arxiv_id        TEXT,
    doi             TEXT,
    url             TEXT,
    title           TEXT NOT NULL,
    abstract        TEXT NOT NULL DEFAULT '',
    year            INT,
    venue_name      TEXT,
    pdf_path        TEXT,
    tei_path        TEXT,
    metadata_source TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX versions_work_id_idx ON versions (work_id);
CREATE UNIQUE INDEX versions_arxiv_id_uidx ON versions (arxiv_id) WHERE arxiv_id IS NOT NULL;
CREATE UNIQUE INDEX versions_doi_uidx ON versions (doi) WHERE doi IS NOT NULL;

ALTER TABLE works
    ADD CONSTRAINT works_primary_version_fk
    FOREIGN KEY (primary_version_id) REFERENCES versions(id) DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE authors (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    full_name    TEXT NOT NULL,
    s2_author_id TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX authors_name_idx ON authors (full_name);
CREATE UNIQUE INDEX authors_s2_uidx ON authors (s2_author_id) WHERE s2_author_id IS NOT NULL;

CREATE TABLE version_authors (
    version_id UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    author_id  UUID NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    position   INT  NOT NULL DEFAULT 0,
    PRIMARY KEY (version_id, author_id)
);

CREATE TABLE work_projects (
    work_id    UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    PRIMARY KEY (work_id, project_id)
);

CREATE TABLE citations (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    citing_work_id  UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    cited_work_id   UUID REFERENCES works(id) ON DELETE SET NULL,
    cited_external  JSONB,  -- when cited paper not yet in our library
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX citations_citing_idx ON citations (citing_work_id);
CREATE INDEX citations_cited_idx ON citations (cited_work_id);

CREATE TABLE merge_history (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kept_work_id   UUID NOT NULL REFERENCES works(id),
    merged_work_id UUID NOT NULL,  -- may already be deleted; keep id for audit
    snapshot       JSONB NOT NULL,
    merged_by      UUID REFERENCES users(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    reverted_at    TIMESTAMPTZ
);

-- ─── DNA entities ────────────────────────────────────────────────────────────

CREATE TABLE claims (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id       UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    text          TEXT NOT NULL,
    source        source_layer NOT NULL DEFAULT 'ai_candidate',
    review_status review_status NOT NULL DEFAULT 'unreviewed',
    created_by    UUID REFERENCES users(id),
    model_version TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX claims_work_id_idx ON claims (work_id);

CREATE TABLE methods (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id       UUID REFERENCES works(id) ON DELETE CASCADE,
    parent_id     UUID REFERENCES methods(id) ON DELETE SET NULL,
    name          TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    source        source_layer NOT NULL DEFAULT 'ai_candidate',
    review_status review_status NOT NULL DEFAULT 'unreviewed',
    created_by    UUID REFERENCES users(id),
    model_version TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX methods_work_id_idx ON methods (work_id);

CREATE TABLE datasets (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    aliases    TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX datasets_name_uidx ON datasets (lower(name));

CREATE TABLE extractions (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id     UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    model          TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    raw            JSONB,
    status         TEXT NOT NULL DEFAULT 'pending',
    usage          JSONB,
    cost_usd       DOUBLE PRECISION,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX extractions_version_idx ON extractions (version_id);

-- ─── Relations (reified) ─────────────────────────────────────────────────────

CREATE TABLE relations (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    type              relation_type NOT NULL,
    aspect            TEXT,
    scope             TEXT,
    explanation       TEXT NOT NULL DEFAULT '',
    confidence        DOUBLE PRECISION,
    source            source_layer NOT NULL DEFAULT 'ai_candidate',
    review_status     review_status NOT NULL DEFAULT 'unreviewed',
    created_by_user   UUID REFERENCES users(id),
    model_version     TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX relations_type_idx ON relations (type);
CREATE INDEX relations_status_idx ON relations (review_status);

CREATE TABLE relation_members (
    relation_id    UUID NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
    entity_kind    entity_kind NOT NULL,
    entity_id      UUID NOT NULL,
    role           member_role NOT NULL,
    anchor_work_id UUID REFERENCES works(id) ON DELETE SET NULL,
    position       INT NOT NULL DEFAULT 0,
    PRIMARY KEY (relation_id, entity_kind, entity_id, role)
);

CREATE INDEX relation_members_entity_idx ON relation_members (entity_kind, entity_id);
CREATE INDEX relation_members_anchor_idx ON relation_members (anchor_work_id);

CREATE TABLE evidence_spans (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    relation_id      UUID REFERENCES relations(id) ON DELETE CASCADE,
    claim_id         UUID REFERENCES claims(id) ON DELETE CASCADE,
    extraction_field TEXT,
    version_id       UUID NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
    page             INT NOT NULL,
    text             TEXT NOT NULL,
    bbox             JSONB,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (relation_id IS NOT NULL OR claim_id IS NOT NULL OR extraction_field IS NOT NULL)
);

CREATE INDEX evidence_relation_idx ON evidence_spans (relation_id);
CREATE INDEX evidence_claim_idx ON evidence_spans (claim_id);

-- ─── Collaboration ───────────────────────────────────────────────────────────

CREATE TABLE reviews (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_kind subject_kind NOT NULL,
    subject_id   UUID NOT NULL,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    verdict      review_verdict NOT NULL,
    comment      TEXT NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (subject_kind, subject_id, user_id)
);

CREATE INDEX reviews_subject_idx ON reviews (subject_kind, subject_id);

CREATE TABLE claim_judgments (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id     UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    verdict      claim_verdict NOT NULL,
    conditions   TEXT NOT NULL DEFAULT '',
    evidence_url TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX claim_judgments_claim_idx ON claim_judgments (claim_id);

CREATE TABLE reading_status (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    work_id    UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    status     reading_level NOT NULL DEFAULT 'unread',
    starred    BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, work_id)
);

CREATE TABLE annotations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id     UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    version_id  UUID REFERENCES versions(id) ON DELETE SET NULL,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind        annotation_kind NOT NULL DEFAULT 'note',
    visibility  visibility NOT NULL DEFAULT 'team',
    anchor      JSONB,
    body        TEXT NOT NULL,
    parent_id   UUID REFERENCES annotations(id) ON DELETE CASCADE,
    resolved    BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX annotations_work_idx ON annotations (work_id);

-- ─── Distance engine & vectors ───────────────────────────────────────────────

-- D matched to embedding provider; Qwen3-Embedding-8B = 4096 (see 002 migration)
CREATE TABLE embeddings (
    entity_kind entity_kind NOT NULL,
    entity_id   UUID NOT NULL,
    field       TEXT NOT NULL DEFAULT 'default',
    model       TEXT NOT NULL,
    vec         vector(4096) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (entity_kind, entity_id, field)
);

CREATE TABLE neighbors (
    dimension        neighbor_dimension NOT NULL,
    work_id          UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    neighbor_work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    score            DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (dimension, work_id, neighbor_work_id),
    CHECK (work_id <> neighbor_work_id)
);

CREATE INDEX neighbors_work_score_idx ON neighbors (dimension, work_id, score DESC);

CREATE TABLE saved_views (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name       TEXT NOT NULL,
    weights    JSONB NOT NULL,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ─── Jobs & imports ──────────────────────────────────────────────────────────

CREATE TABLE jobs (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind       TEXT NOT NULL,
    payload    JSONB NOT NULL DEFAULT '{}',
    status     job_status NOT NULL DEFAULT 'queued',
    attempts   INT NOT NULL DEFAULT 0,
    run_after  TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_by  TEXT,
    locked_at  TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX jobs_poll_idx ON jobs (status, run_after)
    WHERE status = 'queued';

CREATE TABLE import_batches (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_by UUID REFERENCES users(id),
    raw_input  TEXT NOT NULL,
    parsed     JSONB,
    status     import_status NOT NULL DEFAULT 'preview',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ─── Sessions (tower-sessions-sqlx-store) ────────────────────────────────────
-- Table created by the session store itself; placeholder for completeness.

CREATE TABLE IF NOT EXISTS tower_sessions (
    id TEXT PRIMARY KEY,
    data BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);
