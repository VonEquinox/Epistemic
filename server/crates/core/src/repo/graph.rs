use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    EntityKind, MemberRole, Neighbor, NeighborDimension, RelationType, ReviewStatus, SourceLayer,
};
use crate::error::AppResult;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MapNode {
    pub work_id: Uuid,
    pub title: String,
    pub year: Option<i32>,
    pub readers: i64,
    pub has_dispute: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct NeighborEntry {
    pub neighbor_work_id: Uuid,
    pub score: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MapResponse {
    pub nodes: Vec<MapNode>,
    /// dimension -> work_id -> neighbors
    pub neighbors: std::collections::HashMap<String, std::collections::HashMap<String, Vec<NeighborEntry>>>,
}

#[derive(sqlx::FromRow)]
struct MapNodeRow {
    id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    title: Option<String>,
    year: Option<i32>,
    readers: Option<i64>,
    has_dispute: Option<bool>,
}

pub async fn map_data(pool: &PgPool) -> AppResult<MapResponse> {
    let nodes_raw = sqlx::query_as::<_, MapNodeRow>(
        r#"
        SELECT w.id, w.created_at,
               COALESCE(v.title, w.title_norm) AS title,
               v.year AS year,
               (SELECT COUNT(*) FROM reading_status rs
                WHERE rs.work_id = w.id AND rs.status <> 'unread') AS readers,
               EXISTS(
                   SELECT 1 FROM relation_members rm
                   JOIN relations r ON r.id = rm.relation_id
                   WHERE rm.anchor_work_id = w.id AND r.review_status = 'disputed'
               ) AS has_dispute
        FROM works w
        LEFT JOIN versions v ON v.id = w.primary_version_id
        ORDER BY w.created_at
        "#,
    )
    .fetch_all(pool)
    .await?;

    let nodes: Vec<MapNode> = nodes_raw
        .into_iter()
        .map(|r| MapNode {
            work_id: r.id,
            title: r.title.unwrap_or_default(),
            year: r.year,
            readers: r.readers.unwrap_or(0),
            has_dispute: r.has_dispute.unwrap_or(false),
            created_at: r.created_at,
        })
        .collect();

    let neighbor_rows = sqlx::query_as::<_, Neighbor>(
        r#"
        SELECT dimension, work_id, neighbor_work_id, score
        FROM neighbors
        ORDER BY dimension, work_id, score DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut neighbors: std::collections::HashMap<
        String,
        std::collections::HashMap<String, Vec<NeighborEntry>>,
    > = std::collections::HashMap::new();

    for n in neighbor_rows {
        let dim = serde_json::to_value(n.dimension)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".into());

        neighbors
            .entry(dim)
            .or_default()
            .entry(n.work_id.to_string())
            .or_default()
            .push(NeighborEntry {
                neighbor_work_id: n.neighbor_work_id,
                score: n.score,
            });
    }

    Ok(MapResponse { nodes, neighbors })
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EgoNode {
    pub id: Uuid,
    pub kind: String,
    pub label: String,
    pub work_id: Option<Uuid>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EgoEdge {
    pub relation_id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation_type: RelationType,
    pub review_status: ReviewStatus,
    pub source_layer: SourceLayer,
    pub confidence: Option<f64>,
    pub explanation: String,
    pub review_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EgoResponse {
    pub center: EgoNode,
    pub nodes: Vec<EgoNode>,
    pub edges: Vec<EgoEdge>,
}

async fn work_label(pool: &PgPool, work_id: Uuid) -> String {
    match crate::repo::works::get_work(pool, work_id).await {
        Ok(w) => {
            if let Some(vid) = w.primary_version_id {
                crate::repo::works::get_version(pool, vid)
                    .await
                    .map(|v| v.title)
                    .unwrap_or(w.title_norm)
            } else {
                w.title_norm
            }
        }
        Err(_) => work_id.to_string(),
    }
}

pub async fn ego_work(pool: &PgPool, work_id: Uuid, depth: i32) -> AppResult<EgoResponse> {
    let _work = crate::repo::works::get_work(pool, work_id).await?;
    let title = work_label(pool, work_id).await;

    let center = EgoNode {
        id: work_id,
        kind: "work".into(),
        label: title,
        work_id: Some(work_id),
    };

    let rel_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT r.id
        FROM relations r
        JOIN relation_members rm ON rm.relation_id = r.id
        WHERE rm.anchor_work_id = $1
          AND r.review_status <> 'rejected'
          AND r.type <> 'cites'
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;

    let mut nodes = vec![center.clone()];
    let mut edges = Vec::new();
    let mut seen = std::collections::HashSet::new();
    seen.insert(work_id);

    for rid in rel_ids {
        push_relation(pool, rid, &mut nodes, &mut edges, &mut seen).await?;
    }

    if depth >= 2 {
        let hop1: Vec<Uuid> = nodes
            .iter()
            .filter(|n| n.kind == "work" && n.id != work_id)
            .map(|n| n.id)
            .collect();
        for hid in hop1.into_iter().take(15) {
            let more: Vec<Uuid> = sqlx::query_scalar(
                r#"
                SELECT DISTINCT r.id
                FROM relations r
                JOIN relation_members rm ON rm.relation_id = r.id
                WHERE rm.anchor_work_id = $1
                  AND r.review_status <> 'rejected'
                  AND r.type <> 'cites'
                LIMIT 10
                "#,
            )
            .bind(hid)
            .fetch_all(pool)
            .await?;
            for rid in more {
                if edges.iter().any(|e| e.relation_id == rid) {
                    continue;
                }
                push_relation(pool, rid, &mut nodes, &mut edges, &mut seen).await?;
            }
        }
    }

    if nodes.len() > 30 {
        nodes.truncate(30);
        let keep: std::collections::HashSet<Uuid> = nodes.iter().map(|n| n.id).collect();
        edges.retain(|e| keep.contains(&e.source_id) && keep.contains(&e.target_id));
    }

    Ok(EgoResponse {
        center,
        nodes,
        edges,
    })
}

async fn push_relation(
    pool: &PgPool,
    rid: Uuid,
    nodes: &mut Vec<EgoNode>,
    edges: &mut Vec<EgoEdge>,
    seen: &mut std::collections::HashSet<Uuid>,
) -> AppResult<()> {
    let detail = crate::repo::relations::get_relation(pool, rid).await?;
    let r = &detail.relation;

    let source = detail.members.iter().find(|m| m.role == MemberRole::Source);
    let target = detail.members.iter().find(|m| m.role == MemberRole::Target);
    let (Some(src), Some(tgt)) = (source, target) else {
        return Ok(());
    };

    for m in [src, tgt] {
        if seen.insert(m.entity_id) {
            let label = if m.entity_kind == EntityKind::Work {
                work_label(pool, m.entity_id).await
            } else {
                format!("{:?} {}", m.entity_kind, m.entity_id)
            };
            let kind = match m.entity_kind {
                EntityKind::Work => "work",
                EntityKind::Claim => "claim",
                EntityKind::Method => "method",
                EntityKind::Dataset => "dataset",
                EntityKind::Version => "version",
                EntityKind::Person => "person",
            };
            nodes.push(EgoNode {
                id: m.entity_id,
                kind: kind.into(),
                label,
                work_id: m.anchor_work_id.or(
                    if m.entity_kind == EntityKind::Work {
                        Some(m.entity_id)
                    } else {
                        None
                    },
                ),
            });
        }
    }

    edges.push(EgoEdge {
        relation_id: r.id,
        source_id: src.entity_id,
        target_id: tgt.entity_id,
        relation_type: r.relation_type,
        review_status: r.review_status,
        source_layer: r.source,
        confidence: r.confidence,
        explanation: r.explanation.clone(),
        review_count: detail.reviews.len() as i64,
    });
    Ok(())
}

pub async fn upsert_neighbor(
    pool: &PgPool,
    dimension: NeighborDimension,
    work_id: Uuid,
    neighbor_work_id: Uuid,
    score: f64,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO neighbors (dimension, work_id, neighbor_work_id, score)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (dimension, work_id, neighbor_work_id)
        DO UPDATE SET score = EXCLUDED.score
        "#,
    )
    .bind(dimension)
    .bind(work_id)
    .bind(neighbor_work_id)
    .bind(score)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn trim_neighbors(
    pool: &PgPool,
    dimension: NeighborDimension,
    work_id: Uuid,
    k: i64,
) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM neighbors n
        WHERE n.dimension = $1 AND n.work_id = $2
          AND n.neighbor_work_id NOT IN (
              SELECT neighbor_work_id FROM neighbors
              WHERE dimension = $1 AND work_id = $2
              ORDER BY score DESC LIMIT $3
          )
        "#,
    )
    .bind(dimension)
    .bind(work_id)
    .bind(k)
    .execute(pool)
    .await?;
    Ok(())
}
