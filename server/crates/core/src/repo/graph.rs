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

/// Assertion edge projected to Work layer for near-LOD map rendering.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MapEdge {
    pub relation_id: Uuid,
    pub source_work_id: Uuid,
    pub target_work_id: Uuid,
    pub relation_type: RelationType,
    pub review_status: ReviewStatus,
    pub source_layer: SourceLayer,
    pub confidence: Option<f64>,
    pub explanation: String,
    pub review_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MapResponse {
    pub nodes: Vec<MapNode>,
    /// dimension -> work_id -> neighbors
    pub neighbors: std::collections::HashMap<String, std::collections::HashMap<String, Vec<NeighborEntry>>>,
    /// Assertion relations (non-cites, non-rejected) for near-LOD drawing.
    pub edges: Vec<MapEdge>,
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

    // Assertion edges: work→work projection via anchor_work_id (or entity when kind=work).
    // High-risk unreviewed stay in the payload; frontend may filter for display.
    #[derive(sqlx::FromRow)]
    struct EdgeRow {
        relation_id: Uuid,
        source_work_id: Uuid,
        target_work_id: Uuid,
        relation_type: RelationType,
        review_status: ReviewStatus,
        source_layer: SourceLayer,
        confidence: Option<f64>,
        explanation: String,
        review_count: i64,
    }

    let edge_rows = sqlx::query_as::<_, EdgeRow>(
        r#"
        SELECT
            r.id AS relation_id,
            COALESCE(src.anchor_work_id, CASE WHEN src.entity_kind = 'work' THEN src.entity_id END) AS source_work_id,
            COALESCE(tgt.anchor_work_id, CASE WHEN tgt.entity_kind = 'work' THEN tgt.entity_id END) AS target_work_id,
            r.type AS relation_type,
            r.review_status,
            r.source AS source_layer,
            r.confidence,
            r.explanation,
            (SELECT COUNT(*) FROM reviews rv
             WHERE rv.subject_kind = 'relation' AND rv.subject_id = r.id) AS review_count
        FROM relations r
        JOIN relation_members src ON src.relation_id = r.id AND src.role = 'source'
        JOIN relation_members tgt ON tgt.relation_id = r.id AND tgt.role = 'target'
        WHERE r.type <> 'cites'
          AND r.review_status <> 'rejected'
          AND NOT (
            r.type IN ('fails_to_reproduce', 'contradicts_claim')
            AND r.review_status <> 'confirmed'
          )
          AND COALESCE(src.anchor_work_id, CASE WHEN src.entity_kind = 'work' THEN src.entity_id END) IS NOT NULL
          AND COALESCE(tgt.anchor_work_id, CASE WHEN tgt.entity_kind = 'work' THEN tgt.entity_id END) IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;

    let edges: Vec<MapEdge> = edge_rows
        .into_iter()
        .filter(|e| e.source_work_id != e.target_work_id)
        .map(|e| MapEdge {
            relation_id: e.relation_id,
            source_work_id: e.source_work_id,
            target_work_id: e.target_work_id,
            relation_type: e.relation_type,
            review_status: e.review_status,
            source_layer: e.source_layer,
            confidence: e.confidence,
            explanation: e.explanation,
            review_count: e.review_count,
        })
        .collect();

    Ok(MapResponse {
        nodes,
        neighbors,
        edges,
    })
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EgoNode {
    pub id: Uuid,
    pub kind: String,
    pub label: String,
    pub work_id: Option<Uuid>,
    /// Present on semantic-group overflow nodes ("improves_on:in").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_count: Option<i64>,
    /// Selection relevance score (debug / UI).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
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
    /// Bundle key: "{src}|{tgt}|{semantic_group}" for frontend bundling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_key: Option<String>,
}

/// Semantic group overflow node (relation_type × direction).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EgoGroup {
    pub key: String,
    pub relation_type: RelationType,
    /// "in" = others → center, "out" = center → others
    pub direction: String,
    pub count: i64,
    pub member_work_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EgoResponse {
    pub center: EgoNode,
    pub nodes: Vec<EgoNode>,
    pub edges: Vec<EgoEdge>,
    pub groups: Vec<EgoGroup>,
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

/// Semantic group of a relation type for bundling / overflow.
fn semantic_group(t: RelationType) -> &'static str {
    match t {
        RelationType::UsesMethodFrom | RelationType::ImprovesOn | RelationType::AlternativeTo => {
            "method"
        }
        RelationType::UsesDatasetFrom
        | RelationType::ComparesAgainst
        | RelationType::Reproduces
        | RelationType::FailsToReproduce => "experiment",
        RelationType::SupportsClaim | RelationType::ContradictsClaim => "argument",
        RelationType::PrerequisiteFor => "reading",
        RelationType::Cites | RelationType::VersionOf => "meta",
    }
}

fn mode_weight(mode: &str, t: RelationType) -> f64 {
    match mode {
        "prerequisite" => {
            if matches!(t, RelationType::PrerequisiteFor) {
                2.0
            } else if t.is_method_lineage() {
                1.2
            } else {
                0.8
            }
        }
        "dispute" => {
            if matches!(
                t,
                RelationType::ContradictsClaim | RelationType::FailsToReproduce
            ) {
                2.0
            } else {
                0.7
            }
        }
        "evolution" => {
            if matches!(t, RelationType::ImprovesOn | RelationType::UsesMethodFrom) {
                1.8
            } else if t.is_method_lineage() {
                1.3
            } else {
                0.8
            }
        }
        _ => 1.0, // default / explore
    }
}

fn status_weight(s: ReviewStatus) -> f64 {
    match s {
        ReviewStatus::Confirmed => 3.0,
        ReviewStatus::Disputed => 2.5,
        ReviewStatus::Unreviewed => 1.0,
        ReviewStatus::Rejected => 0.0,
    }
}

#[derive(Clone)]
struct CandEdge {
    relation_id: Uuid,
    source_id: Uuid,
    target_id: Uuid,
    relation_type: RelationType,
    review_status: ReviewStatus,
    source_layer: SourceLayer,
    confidence: Option<f64>,
    explanation: String,
    review_count: i64,
    other_work: Uuid,
    direction: &'static str, // "in" | "out"
    score: f64,
}

/// Ego view for a work: select ≤30 nodes by relevance, overflow → semantic groups.
pub async fn ego_work(pool: &PgPool, work_id: Uuid, depth: i32) -> AppResult<EgoResponse> {
    ego_work_mode(pool, work_id, depth, "explore").await
}

pub async fn ego_work_mode(
    pool: &PgPool,
    work_id: Uuid,
    depth: i32,
    mode: &str,
) -> AppResult<EgoResponse> {
    let _work = crate::repo::works::get_work(pool, work_id).await?;
    let title = work_label(pool, work_id).await;

    let center = EgoNode {
        id: work_id,
        kind: "work".into(),
        label: title,
        work_id: Some(work_id),
        group_key: None,
        group_count: None,
        score: None,
    };

    // Collect candidate edges anchored on this work (and hop-2 if requested).
    let mut cand: Vec<CandEdge> = Vec::new();
    collect_cands(pool, work_id, work_id, mode, &mut cand).await?;

    if depth >= 2 {
        // Take top others from hop-1 by score for expansion
        let mut hop1: Vec<(Uuid, f64)> = cand
            .iter()
            .map(|c| (c.other_work, c.score))
            .collect();
        hop1.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hop1.dedup_by_key(|x| x.0);
        for (hid, _) in hop1.into_iter().take(12) {
            collect_cands(pool, hid, work_id, mode, &mut cand).await?;
        }
    }

    // Best score per other work
    let mut best: std::collections::HashMap<Uuid, f64> = std::collections::HashMap::new();
    for c in &cand {
        best.entry(c.other_work)
            .and_modify(|s| *s = s.max(c.score))
            .or_insert(c.score);
    }
    let mut ranked: Vec<(Uuid, f64)> = best.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Keep top 29 others (+ center = 30). Overflow → groups by (type × direction).
    const MAX_OTHERS: usize = 29;
    let kept: std::collections::HashSet<Uuid> = ranked
        .iter()
        .take(MAX_OTHERS)
        .map(|(id, _)| *id)
        .collect();
    let overflow: Vec<(Uuid, f64)> = ranked.into_iter().skip(MAX_OTHERS).collect();

    // Build overflow groups from edges whose other_work is overflowed
    let mut group_map: std::collections::HashMap<String, EgoGroup> =
        std::collections::HashMap::new();
    for c in &cand {
        if kept.contains(&c.other_work) {
            continue;
        }
        if !overflow.iter().any(|(id, _)| *id == c.other_work) {
            continue;
        }
        let key = format!("{}:{}", c.relation_type.as_str(), c.direction);
        let g = group_map.entry(key.clone()).or_insert_with(|| EgoGroup {
            key: key.clone(),
            relation_type: c.relation_type,
            direction: c.direction.into(),
            count: 0,
            member_work_ids: vec![],
        });
        if !g.member_work_ids.contains(&c.other_work) {
            g.member_work_ids.push(c.other_work);
            g.count = g.member_work_ids.len() as i64;
        }
    }
    let groups: Vec<EgoGroup> = {
        let mut g: Vec<_> = group_map.into_values().collect();
        g.sort_by(|a, b| b.count.cmp(&a.count));
        g
    };

    // Nodes: center + kept works + group nodes
    let mut nodes = vec![center.clone()];
    let mut seen = std::collections::HashSet::new();
    seen.insert(work_id);
    for (oid, sc) in ranked_kept(&cand, &kept) {
        if seen.insert(oid) {
            nodes.push(EgoNode {
                id: oid,
                kind: "work".into(),
                label: work_label(pool, oid).await,
                work_id: Some(oid),
                group_key: None,
                group_count: None,
                score: Some(sc),
            });
        }
    }
    // Deterministic UUIDs for group nodes via uuid v5-like from key hash
    for g in &groups {
        let gid = group_node_id(work_id, &g.key);
        nodes.push(EgoNode {
            id: gid,
            kind: "group".into(),
            label: group_label(g),
            work_id: None,
            group_key: Some(g.key.clone()),
            group_count: Some(g.count),
            score: None,
        });
    }

    // Edges among center/kept; edges to groups for overflow representatives
    let mut edges = Vec::new();
    let mut seen_rel = std::collections::HashSet::new();
    for c in &cand {
        if !seen_rel.insert(c.relation_id) {
            continue;
        }
        let src_kept = c.source_id == work_id || kept.contains(&c.source_id);
        let tgt_kept = c.target_id == work_id || kept.contains(&c.target_id);
        if src_kept && tgt_kept {
            let bk = format!(
                "{}|{}|{}",
                c.source_id,
                c.target_id,
                semantic_group(c.relation_type)
            );
            edges.push(EgoEdge {
                relation_id: c.relation_id,
                source_id: c.source_id,
                target_id: c.target_id,
                relation_type: c.relation_type,
                review_status: c.review_status,
                source_layer: c.source_layer,
                confidence: c.confidence,
                explanation: c.explanation.clone(),
                review_count: c.review_count,
                bundle_key: Some(bk),
            });
        }
    }
    // One visual edge per group to center
    for g in &groups {
        let gid = group_node_id(work_id, &g.key);
        let (source_id, target_id) = if g.direction == "out" {
            (work_id, gid)
        } else {
            (gid, work_id)
        };
        edges.push(EgoEdge {
            relation_id: gid, // synthetic
            source_id,
            target_id,
            relation_type: g.relation_type,
            review_status: ReviewStatus::Unreviewed,
            source_layer: SourceLayer::AiCandidate,
            confidence: None,
            explanation: format!("{} 篇溢出", g.count),
            review_count: 0,
            bundle_key: None,
        });
    }

    Ok(EgoResponse {
        center,
        nodes,
        edges,
        groups,
    })
}

fn ranked_kept(
    cand: &[CandEdge],
    kept: &std::collections::HashSet<Uuid>,
) -> Vec<(Uuid, f64)> {
    let mut best: std::collections::HashMap<Uuid, f64> = std::collections::HashMap::new();
    for c in cand {
        if kept.contains(&c.other_work) {
            best.entry(c.other_work)
                .and_modify(|s| *s = s.max(c.score))
                .or_insert(c.score);
        }
    }
    let mut v: Vec<_> = best.into_iter().collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    v
}

fn group_label(g: &EgoGroup) -> String {
    let dir = if g.direction == "in" {
        "指向它的"
    } else {
        "它指出的"
    };
    format!("{} {} +{}", dir, g.relation_type.as_str().replace('_', " "), g.count)
}

fn group_node_id(center: Uuid, key: &str) -> Uuid {
    // Stable synthetic id from center + key (not a real entity).
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    center.hash(&mut h);
    key.hash(&mut h);
    let n = h.finish();
    // Build a UUID from hash bytes (version bits ignored — display only).
    let bytes = n.to_le_bytes();
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&bytes);
    b[8..].copy_from_slice(&bytes);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Uuid::from_bytes(b)
}

async fn collect_cands(
    pool: &PgPool,
    anchor: Uuid,
    center: Uuid,
    mode: &str,
    out: &mut Vec<CandEdge>,
) -> AppResult<()> {
    let rel_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT r.id
        FROM relations r
        JOIN relation_members rm ON rm.relation_id = r.id
        WHERE rm.anchor_work_id = $1
          AND r.review_status <> 'rejected'
          AND r.type <> 'cites'
          AND NOT (
            r.type IN ('fails_to_reproduce', 'contradicts_claim')
            AND r.review_status <> 'confirmed'
          )
        "#,
    )
    .bind(anchor)
    .fetch_all(pool)
    .await?;

    for rid in rel_ids {
        if out.iter().any(|c| c.relation_id == rid) {
            continue;
        }
        let detail = crate::repo::relations::get_relation(pool, rid).await?;
        let r = &detail.relation;
        let source = detail.members.iter().find(|m| m.role == MemberRole::Source);
        let target = detail.members.iter().find(|m| m.role == MemberRole::Target);
        let (Some(src), Some(tgt)) = (source, target) else {
            continue;
        };
        // Project to work ids
        let src_w = src
            .anchor_work_id
            .or(if src.entity_kind == EntityKind::Work {
                Some(src.entity_id)
            } else {
                None
            });
        let tgt_w = tgt
            .anchor_work_id
            .or(if tgt.entity_kind == EntityKind::Work {
                Some(tgt.entity_id)
            } else {
                None
            });
        let (Some(sw), Some(tw)) = (src_w, tgt_w) else {
            continue;
        };
        if sw == tw {
            continue;
        }
        // Determine "other" relative to center (or anchor when expanding)
        let (other, direction) = if sw == center {
            (tw, "out")
        } else if tw == center {
            (sw, "in")
        } else if sw == anchor {
            (tw, "out")
        } else if tw == anchor {
            (sw, "in")
        } else {
            // hop-2 edge not touching center/anchor as endpoint — still keep if one side known
            continue;
        };

        let conf = r.confidence.unwrap_or(0.6);
        let score = status_weight(r.review_status)
            * mode_weight(mode, r.relation_type)
            * (1.0 + conf)
            * if r.relation_type == RelationType::Cites {
                0.3
            } else {
                1.0
            }
            + (detail.reviews.len() as f64) * 0.2;

        // Team signal: readers on other work
        let readers: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM reading_status
            WHERE work_id = $1 AND status <> 'unread'
            "#,
        )
        .bind(other)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        let score = score + (readers as f64) * 0.15;

        out.push(CandEdge {
            relation_id: r.id,
            source_id: sw,
            target_id: tw,
            relation_type: r.relation_type,
            review_status: r.review_status,
            source_layer: r.source,
            confidence: r.confidence,
            explanation: r.explanation.clone(),
            review_count: detail.reviews.len() as i64,
            other_work: other,
            direction,
            score,
        });
    }
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
