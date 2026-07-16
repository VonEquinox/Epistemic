# Epistemic MCP

The MCP server is read-only and runs over stdio. It uses a personal token, so every Codex instance sees only the groups, graphs, private comments, and team comments available to that Epistemic user.

## Tools

- `help` — full usage guide; call it when unsure.
- `list_graphs` — groups and graphs visible to the current user.
- `get_graph_snapshot` — paged nodes, similarity neighbors, assertion edges, and visible comment counts (`offset`/`limit`).
- `get_node_context` — compact paper metadata, DNA, evidence, comments, and relation summary.
- `get_node_comments` — member comments/ideas/thinking/reviews; supports `since`, kind filters, and a limit.
- `get_node_source` — stored HTML/TEI text or text extracted from selected PDF pages.
- `get_node_relations` — directed assertion relations, evidence/reviews, citations, similarity neighbors, and nearby node titles.

## Setup

1. Open **Settings → Codex / MCP access token** and create a token. It is shown once.
2. Build the server:

```bash
cd server
cargo build --release -p epistemic-mcp
```

3. Register it with Codex (replace paths and token):

```bash
codex mcp add epistemic \
  --env EPISTEMIC_MCP_TOKEN=epm_xxx \
  --env DATABASE_URL=postgres://epistemic:epistemic@localhost:5432/epistemic \
  --env PDF_DIR=/absolute/path/to/Epistemic/data/pdfs \
  --env TEI_DIR=/absolute/path/to/Epistemic/data/tei \
  -- /absolute/path/to/Epistemic/server/target/release/epistemic-mcp
```

Use `codex mcp list` to verify the entry. Revoke a token from Settings if it is exposed or no longer needed.
