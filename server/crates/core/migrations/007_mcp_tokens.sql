-- Personal access tokens for local/remote MCP clients.

CREATE TABLE mcp_access_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL DEFAULT 'Codex',
    token_hash   TEXT NOT NULL UNIQUE,
    last_used_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX mcp_access_tokens_user_idx
    ON mcp_access_tokens (user_id, created_at DESC);
