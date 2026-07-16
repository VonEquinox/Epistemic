use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use epistemic_core::domain::{EntityKind, MemberRole, NodeComment, UserPublic, Version};
use epistemic_core::repo::{comments, graph, groups, mcp_tokens, relations, works};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{
    model::CallToolResult, schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::process::Command;
use uuid::Uuid;

const HELP_TEXT: &str = r#"Epistemic MCP exposes a user's research-group graphs.
Recommended flow: list_graphs -> get_graph_snapshot -> get_node_context. Use get_node_comments for member ideas/reviews, get_node_source for paper text, and get_node_relations for graph edges and nearby papers.
All graph tools require graph_id and enforce the token owner's group membership. Private comments are visible only to their author. Tool results preserve IDs, authors, timestamps, evidence, and relation direction."#;

#[derive(Clone)]
struct EpistemicMcp {
    pool: PgPool,
    user: UserPublic,
    pdf_dir: Arc<PathBuf>,
    tei_dir: Arc<PathBuf>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct HelpParams {
    /// all | workflow | comments | source | relations
    topic: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GraphSnapshotParams {
    graph_id: String,
    /// Zero-based node offset. Default 0.
    offset: Option<usize>,
    /// Maximum nodes returned. Default 500, maximum 2000.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct NodeParams {
    graph_id: String,
    work_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct NodeCommentsParams {
    graph_id: String,
    work_id: String,
    /// Optional RFC3339 timestamp; only comments updated after it are returned.
    since: Option<String>,
    /// Optional kinds: comment, idea, thinking, review, question, critique.
    kinds: Option<Vec<String>>,
    /// Default 200, maximum 1000.
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct NodeSourceParams {
    graph_id: String,
    work_id: String,
    version_id: Option<String>,
    /// First PDF page, 1-based.
    page_start: Option<u32>,
    /// Last PDF page, inclusive.
    page_end: Option<u32>,
    /// Default 50000, maximum 200000.
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct NodeRelationsParams {
    graph_id: String,
    work_id: String,
    /// both | incoming | outgoing
    direction: Option<String>,
    /// Optional relation type filters.
    relation_types: Option<Vec<String>>,
}

#[tool_router(router = tool_router)]
impl EpistemicMcp {
    fn new(pool: PgPool, user: UserPublic, pdf_dir: PathBuf, tei_dir: PathBuf) -> Self {
        Self {
            pool,
            user,
            pdf_dir: Arc::new(pdf_dir),
            tei_dir: Arc::new(tei_dir),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(name = "help", description = "Explain how to use Epistemic MCP.")]
    async fn help(&self, Parameters(params): Parameters<HelpParams>) -> CallToolResult {
        let topic = params.topic.as_deref().unwrap_or("all");
        let detail = match topic {
            "workflow" => "Call list_graphs, choose graph_id, inspect get_graph_snapshot, then use get_node_context and the specialized tools.",
            "comments" => "get_node_comments returns visible member comments with author, kind, visibility and timestamps. Use since for incremental reads.",
            "source" => "get_node_source returns stored HTML/TEI text or extracts text from the stored PDF. Use page_start/page_end and max_chars to control context size.",
            "relations" => "get_node_relations returns directed assertion relations, evidence, reviews, and incoming/outgoing citations, filtered to the selected graph.",
            _ => HELP_TEXT,
        };
        CallToolResult::structured(json!({
            "topic": topic,
            "guide": detail,
            "tools": {
                "list_graphs": "List graphs visible to this user.",
                "get_graph_snapshot": "Read the graph's nodes, neighbors and edges.",
                "get_node_context": "Read a compact paper-node context bundle.",
                "get_node_comments": "Read member comments, ideas, thinking and reviews.",
                "get_node_source": "Read stored paper text or selected PDF pages.",
                "get_node_relations": "Read directed relations and nearby nodes."
            }
        }))
    }

    #[tool(
        name = "list_graphs",
        description = "List graphs visible to this user."
    )]
    async fn list_graphs(&self) -> Result<CallToolResult, String> {
        let group_rows = groups::list_for_user(&self.pool, self.user.id)
            .await
            .map_err(err_string)?;
        let mut output = Vec::new();
        for group in group_rows {
            let graphs = groups::list_graphs(&self.pool, group.group.id)
                .await
                .map_err(err_string)?;
            output.push(json!({
                "group": group,
                "graphs": graphs,
            }));
        }
        Ok(CallToolResult::structured(
            json!({ "user": self.user, "groups": output }),
        ))
    }

    #[tool(name = "get_graph_snapshot", description = "Read a graph snapshot.")]
    async fn get_graph_snapshot(
        &self,
        Parameters(params): Parameters<GraphSnapshotParams>,
    ) -> Result<CallToolResult, String> {
        let graph_id = parse_uuid(&params.graph_id, "graph_id")?;
        let graph_meta = self.require_graph(graph_id).await?;
        let mut snapshot = graph::map_data(&self.pool, Some(graph_id))
            .await
            .map_err(err_string)?;
        let total_nodes = snapshot.nodes.len();
        let offset = params.offset.unwrap_or(0).min(total_nodes);
        let limit = params.limit.unwrap_or(500).clamp(1, 2000);
        snapshot.nodes = snapshot
            .nodes
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();
        let next_offset =
            (offset + snapshot.nodes.len() < total_nodes).then_some(offset + snapshot.nodes.len());
        let truncated = next_offset.is_some();
        let included: HashSet<Uuid> = snapshot.nodes.iter().map(|node| node.work_id).collect();
        snapshot.edges.retain(|edge| {
            included.contains(&edge.source_work_id) && included.contains(&edge.target_work_id)
        });
        for table in snapshot.neighbors.values_mut() {
            table.retain(|work_id, _| {
                Uuid::parse_str(work_id).is_ok_and(|id| included.contains(&id))
            });
            for entries in table.values_mut() {
                entries.retain(|entry| included.contains(&entry.neighbor_work_id));
            }
        }
        let comment_counts = comment_counts(&self.pool, graph_id, self.user.id, &included)
            .await
            .map_err(err_string)?;
        Ok(CallToolResult::structured(json!({
            "graph": graph_meta,
            "total_nodes": total_nodes,
            "offset": offset,
            "next_offset": next_offset,
            "truncated": truncated,
            "comment_counts": comment_counts,
            "snapshot": snapshot,
        })))
    }

    #[tool(
        name = "get_node_context",
        description = "Read compact context for a graph node."
    )]
    async fn get_node_context(
        &self,
        Parameters(params): Parameters<NodeParams>,
    ) -> Result<CallToolResult, String> {
        let graph_id = parse_uuid(&params.graph_id, "graph_id")?;
        let work_id = parse_uuid(&params.work_id, "work_id")?;
        self.require_node(graph_id, work_id).await?;
        let card = works::get_work_card(&self.pool, work_id)
            .await
            .map_err(err_string)?;
        let comments = comments::list_for_node(&self.pool, graph_id, work_id, self.user.id)
            .await
            .map_err(err_string)?;
        let relation_summary = self
            .node_relations_value(graph_id, work_id, "both", None)
            .await?;
        Ok(CallToolResult::structured(json!({
            "work": card.work,
            "primary_version": card.primary_version,
            "versions": card.versions,
            "authors": card.authors,
            "claims": card.claims,
            "methods": card.methods,
            "aspects": card.aspects,
            "evidence": card.evidence,
            "comments": comments.into_iter().rev().take(50).collect::<Vec<_>>(),
            "relations": relation_summary,
        })))
    }

    #[tool(
        name = "get_node_comments",
        description = "Read member comments for a graph node."
    )]
    async fn get_node_comments(
        &self,
        Parameters(params): Parameters<NodeCommentsParams>,
    ) -> Result<CallToolResult, String> {
        let graph_id = parse_uuid(&params.graph_id, "graph_id")?;
        let work_id = parse_uuid(&params.work_id, "work_id")?;
        self.require_node(graph_id, work_id).await?;
        let since = params
            .since
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|error| format!("invalid since timestamp: {error}"))?
            .map(|time| time.with_timezone(&Utc));
        let kinds: Option<HashSet<String>> = params.kinds.map(|items| {
            items
                .into_iter()
                .map(|item| item.to_ascii_lowercase())
                .collect()
        });
        let limit = params.limit.unwrap_or(200).clamp(1, 1000);
        let rows = comments::list_for_node(&self.pool, graph_id, work_id, self.user.id)
            .await
            .map_err(err_string)?;
        let mut filtered: Vec<NodeComment> = rows
            .into_iter()
            .filter(|comment| since.is_none_or(|time| comment.updated_at > time))
            .filter(|comment| {
                kinds.as_ref().is_none_or(|allowed| {
                    serde_json::to_value(comment.kind)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .is_some_and(|kind| allowed.contains(&kind))
                })
            })
            .collect();
        filtered.sort_by_key(|comment| std::cmp::Reverse(comment.updated_at));
        filtered.truncate(limit);
        Ok(CallToolResult::structured(json!({
            "graph_id": graph_id,
            "work_id": work_id,
            "newest_first": true,
            "comments": filtered,
        })))
    }

    #[tool(
        name = "get_node_source",
        description = "Read stored paper text or selected pages."
    )]
    async fn get_node_source(
        &self,
        Parameters(params): Parameters<NodeSourceParams>,
    ) -> Result<CallToolResult, String> {
        let graph_id = parse_uuid(&params.graph_id, "graph_id")?;
        let work_id = parse_uuid(&params.work_id, "work_id")?;
        let version_id = params
            .version_id
            .as_deref()
            .map(|id| parse_uuid(id, "version_id"))
            .transpose()?;
        self.require_node(graph_id, work_id).await?;
        let work = works::get_work(&self.pool, work_id)
            .await
            .map_err(err_string)?;
        let versions = works::list_versions_for_work(&self.pool, work_id)
            .await
            .map_err(err_string)?;
        let version = choose_version(&versions, version_id, work.primary_version_id)?;
        let max_chars = params.max_chars.unwrap_or(50_000).clamp(1_000, 200_000);
        let (source_kind, raw_text) = self
            .read_source_text(version, params.page_start, params.page_end)
            .await?;
        let original_chars = raw_text.chars().count();
        let text: String = raw_text.chars().take(max_chars).collect();
        Ok(CallToolResult::structured(json!({
            "graph_id": graph_id,
            "work_id": work_id,
            "version_id": version.id,
            "title": version.title,
            "source_kind": source_kind,
            "page_start": params.page_start,
            "page_end": params.page_end,
            "original_chars": original_chars,
            "truncated": original_chars > max_chars,
            "text": text,
        })))
    }

    #[tool(
        name = "get_node_relations",
        description = "Read relations around a graph node."
    )]
    async fn get_node_relations(
        &self,
        Parameters(params): Parameters<NodeRelationsParams>,
    ) -> Result<CallToolResult, String> {
        let graph_id = parse_uuid(&params.graph_id, "graph_id")?;
        let work_id = parse_uuid(&params.work_id, "work_id")?;
        self.require_node(graph_id, work_id).await?;
        let direction = params.direction.as_deref().unwrap_or("both");
        Ok(CallToolResult::structured(
            self.node_relations_value(graph_id, work_id, direction, params.relation_types)
                .await?,
        ))
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "epistemic",
    version = "0.1.0",
    instructions = "Research graph context. Call help if tool usage is unclear. All data is scoped to the authenticated Epistemic user."
)]
impl ServerHandler for EpistemicMcp {}

impl EpistemicMcp {
    async fn require_graph(&self, graph_id: Uuid) -> Result<epistemic_core::domain::Graph, String> {
        groups::require_graph_access(&self.pool, graph_id, self.user.id)
            .await
            .map_err(err_string)
    }

    async fn require_node(&self, graph_id: Uuid, work_id: Uuid) -> Result<(), String> {
        self.require_graph(graph_id).await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM graph_works WHERE graph_id = $1 AND work_id = $2)",
        )
        .bind(graph_id)
        .bind(work_id)
        .fetch_one(&self.pool)
        .await
        .map_err(err_string)?;
        if !exists {
            return Err(format!("work {work_id} is not in graph {graph_id}"));
        }
        Ok(())
    }

    async fn read_source_text(
        &self,
        version: &Version,
        page_start: Option<u32>,
        page_end: Option<u32>,
    ) -> Result<(&'static str, String), String> {
        let wants_pages = page_start.is_some() || page_end.is_some();
        if !wants_pages {
            if let Some(result) = self.read_markup_source(version).await? {
                return Ok(result);
            }
        }
        if let Some(rel) = &version.pdf_path {
            let path = safe_stored_path(self.pdf_dir.as_ref(), rel)?;
            if path.exists() {
                let mut command = Command::new("pdftotext");
                if let Some(start) = page_start {
                    command.arg("-f").arg(start.max(1).to_string());
                }
                if let Some(end) = page_end {
                    if let Some(start) = page_start {
                        if end < start {
                            return Err("page_end must be >= page_start".into());
                        }
                    }
                    command.arg("-l").arg(end.max(1).to_string());
                }
                let output = command
                    .arg(&path)
                    .arg("-")
                    .output()
                    .await
                    .map_err(|error| format!("failed to run pdftotext: {error}"))?;
                if !output.status.success() {
                    return Err(format!(
                        "pdftotext failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                return Ok(("pdf", String::from_utf8_lossy(&output.stdout).into_owned()));
            }
        }
        if let Some(result) = self.read_markup_source(version).await? {
            return Ok(result);
        }
        Ok((
            "metadata",
            format!("{}\n\n{}", version.title, version.abstract_text),
        ))
    }

    async fn read_markup_source(
        &self,
        version: &Version,
    ) -> Result<Option<(&'static str, String)>, String> {
        let Some(rel) = &version.tei_path else {
            return Ok(None);
        };
        let (kind, base) = if rel.ends_with(".html") {
            ("html", self.pdf_dir.as_ref())
        } else {
            ("tei", self.tei_dir.as_ref())
        };
        let path = safe_stored_path(base, rel)?;
        if !path.exists() {
            return Ok(None);
        }
        let source = tokio::fs::read_to_string(path).await.map_err(err_string)?;
        Ok(Some((kind, strip_markup(&source))))
    }

    async fn node_relations_value(
        &self,
        graph_id: Uuid,
        work_id: Uuid,
        direction: &str,
        relation_types: Option<Vec<String>>,
    ) -> Result<Value, String> {
        if !matches!(direction, "both" | "incoming" | "outgoing") {
            return Err("direction must be both, incoming, or outgoing".into());
        }
        let graph_work_ids: HashSet<Uuid> = groups::list_graph_work_ids(&self.pool, graph_id)
            .await
            .map_err(err_string)?
            .into_iter()
            .collect();
        let allowed_types: Option<HashSet<String>> = relation_types.map(|items| {
            items
                .into_iter()
                .map(|item| item.to_ascii_lowercase())
                .collect()
        });
        let details = relations::list_for_work(&self.pool, work_id)
            .await
            .map_err(err_string)?;
        let mut assertions = Vec::new();
        let mut related_ids = HashSet::new();
        for detail in details {
            let relation_type = detail.relation.relation_type.as_str();
            if allowed_types
                .as_ref()
                .is_some_and(|allowed| !allowed.contains(relation_type))
            {
                continue;
            }
            let source = member_anchor(&detail.members, MemberRole::Source);
            let target = member_anchor(&detail.members, MemberRole::Target);
            let is_outgoing = source == Some(work_id);
            let is_incoming = target == Some(work_id);
            if (direction == "outgoing" && !is_outgoing)
                || (direction == "incoming" && !is_incoming)
            {
                continue;
            }
            let anchors: HashSet<Uuid> = detail.members.iter().filter_map(member_work_id).collect();
            if !anchors.iter().all(|id| graph_work_ids.contains(id)) {
                continue;
            }
            related_ids.extend(anchors.into_iter().filter(|id| *id != work_id));
            assertions.push(json!({
                "direction": if is_outgoing { "outgoing" } else if is_incoming { "incoming" } else { "involving" },
                "relation": detail,
            }));
        }

        #[derive(serde::Serialize, sqlx::FromRow)]
        struct CitationRow {
            id: Uuid,
            citing_work_id: Uuid,
            cited_work_id: Uuid,
            created_at: DateTime<Utc>,
        }
        let citations = sqlx::query_as::<_, CitationRow>(
            r#"
            SELECT id, citing_work_id, cited_work_id, created_at
            FROM citations
            WHERE cited_work_id IS NOT NULL
              AND (citing_work_id = $1 OR cited_work_id = $1)
            ORDER BY created_at DESC
            "#,
        )
        .bind(work_id)
        .fetch_all(&self.pool)
        .await
        .map_err(err_string)?
        .into_iter()
        .filter(|citation| {
            graph_work_ids.contains(&citation.citing_work_id)
                && graph_work_ids.contains(&citation.cited_work_id)
                && match direction {
                    "incoming" => citation.cited_work_id == work_id,
                    "outgoing" => citation.citing_work_id == work_id,
                    _ => true,
                }
        })
        .collect::<Vec<_>>();
        for citation in &citations {
            let other = if citation.citing_work_id == work_id {
                citation.cited_work_id
            } else {
                citation.citing_work_id
            };
            related_ids.insert(other);
        }

        #[derive(sqlx::FromRow)]
        struct NeighborRow {
            dimension: String,
            work_id: Uuid,
            neighbor_work_id: Uuid,
            score: f64,
        }
        let neighbor_rows = sqlx::query_as::<_, NeighborRow>(
            r#"
            SELECT dimension::text AS dimension, work_id, neighbor_work_id, score
            FROM neighbors
            WHERE work_id = $1 OR neighbor_work_id = $1
            ORDER BY dimension, score DESC
            "#,
        )
        .bind(work_id)
        .fetch_all(&self.pool)
        .await
        .map_err(err_string)?;
        let mut seen_neighbors = HashSet::new();
        let mut similarity_neighbors = Vec::new();
        for neighbor in neighbor_rows {
            if !graph_work_ids.contains(&neighbor.work_id)
                || !graph_work_ids.contains(&neighbor.neighbor_work_id)
            {
                continue;
            }
            let other = if neighbor.work_id == work_id {
                neighbor.neighbor_work_id
            } else {
                neighbor.work_id
            };
            if !seen_neighbors.insert((neighbor.dimension.clone(), other)) {
                continue;
            }
            related_ids.insert(other);
            similarity_neighbors.push(json!({
                "dimension": neighbor.dimension,
                "neighbor_work_id": other,
                "score": neighbor.score,
            }));
        }

        let related_nodes = work_titles(&self.pool, &related_ids)
            .await
            .map_err(err_string)?;
        Ok(json!({
            "graph_id": graph_id,
            "work_id": work_id,
            "assertions": assertions,
            "citations": citations,
            "similarity_neighbors": similarity_neighbors,
            "related_nodes": related_nodes,
        }))
    }
}

fn member_work_id(member: &epistemic_core::domain::RelationMember) -> Option<Uuid> {
    member
        .anchor_work_id
        .or_else(|| (member.entity_kind == EntityKind::Work).then_some(member.entity_id))
}

fn member_anchor(
    members: &[epistemic_core::domain::RelationMember],
    role: MemberRole,
) -> Option<Uuid> {
    members
        .iter()
        .find(|member| member.role == role)
        .and_then(member_work_id)
}

fn choose_version(
    versions: &[Version],
    requested: Option<Uuid>,
    primary: Option<Uuid>,
) -> Result<&Version, String> {
    if let Some(id) = requested.or(primary) {
        return versions
            .iter()
            .find(|version| version.id == id)
            .ok_or_else(|| format!("version {id} does not belong to this work"));
    }
    versions
        .first()
        .ok_or_else(|| "work has no versions".to_string())
}

fn safe_stored_path(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err("stored source path is invalid".into());
    }
    Ok(base.join(path))
}

fn strip_markup(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn comment_counts(
    pool: &PgPool,
    graph_id: Uuid,
    viewer_id: Uuid,
    work_ids: &HashSet<Uuid>,
) -> anyhow::Result<HashMap<Uuid, i64>> {
    if work_ids.is_empty() {
        return Ok(HashMap::new());
    }
    #[derive(sqlx::FromRow)]
    struct Row {
        work_id: Uuid,
        count: i64,
    }
    let ids: Vec<Uuid> = work_ids.iter().copied().collect();
    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT work_id, COUNT(*) AS count
        FROM node_comments
        WHERE graph_id = $1 AND work_id = ANY($2)
          AND (visibility = 'team' OR user_id = $3)
        GROUP BY work_id
        "#,
    )
    .bind(graph_id)
    .bind(&ids)
    .bind(viewer_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.work_id, row.count))
        .collect())
}

async fn work_titles(pool: &PgPool, work_ids: &HashSet<Uuid>) -> anyhow::Result<Vec<Value>> {
    if work_ids.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        title: String,
        year: Option<i32>,
    }
    let ids: Vec<Uuid> = work_ids.iter().copied().collect();
    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT w.id, COALESCE(v.title, w.title_norm) AS title, v.year
        FROM works w
        LEFT JOIN versions v ON v.id = w.primary_version_id
        WHERE w.id = ANY($1)
        ORDER BY title
        "#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| json!({ "work_id": row.id, "title": row.title, "year": row.year }))
        .collect())
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|error| format!("invalid {field}: {error}"))
}

fn err_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_uuid, safe_stored_path, strip_markup};
    use std::path::Path;

    #[test]
    fn source_helpers_reject_traversal_and_strip_markup() {
        assert!(safe_stored_path(Path::new("/tmp/base"), "../secret").is_err());
        assert_eq!(strip_markup("<p>Hello <b>world</b></p>"), "Hello world");
        assert!(parse_uuid("not-a-uuid", "work_id").is_err());
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("epistemic_mcp=info".parse()?),
        )
        .with_writer(std::io::stderr)
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://epistemic:epistemic@localhost:5432/epistemic".into());
    let token = std::env::var("EPISTEMIC_MCP_TOKEN")
        .map_err(|_| anyhow::anyhow!("EPISTEMIC_MCP_TOKEN is required"))?;
    let pdf_dir = PathBuf::from(std::env::var("PDF_DIR").unwrap_or_else(|_| "./data/pdfs".into()));
    let tei_dir = PathBuf::from(std::env::var("TEI_DIR").unwrap_or_else(|_| "./data/tei".into()));

    let pool = epistemic_core::connect_no_migrate(&database_url).await?;
    let user = mcp_tokens::authenticate(&pool, &token).await?;
    tracing::info!(user_id = %user.id, email = %user.email, "starting Epistemic MCP");

    let service = EpistemicMcp::new(pool, user, pdf_dir, tei_dir)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
