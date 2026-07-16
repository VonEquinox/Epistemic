-- Per-user comments attached to a paper node within a graph.

CREATE TYPE comment_kind AS ENUM (
    'comment',
    'idea',
    'thinking',
    'review',
    'question',
    'critique'
);

CREATE TABLE node_comments (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    graph_id    UUID NOT NULL REFERENCES graphs(id) ON DELETE CASCADE,
    work_id     UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind        comment_kind NOT NULL DEFAULT 'comment',
    visibility  visibility NOT NULL DEFAULT 'team',
    body        TEXT NOT NULL CHECK (length(btrim(body)) > 0),
    parent_id   UUID REFERENCES node_comments(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX node_comments_graph_work_idx
    ON node_comments (graph_id, work_id, created_at);
CREATE INDEX node_comments_user_idx
    ON node_comments (user_id, updated_at DESC);
