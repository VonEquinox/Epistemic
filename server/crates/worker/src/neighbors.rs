use crate::jobs::{work_id, JobContext};
use epistemic_core::domain::{NeighborDimension, Job};
use epistemic_core::repo::graph;
use uuid::Uuid;

/// Bibliographic coupling + co-citation, write top-32 neighbors.
pub async fn update_citation(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let wid = work_id(job).ok_or_else(|| anyhow::anyhow!("work_id required"))?;

    // bibliographic coupling: shared outgoing references
    #[derive(sqlx::FromRow)]
    struct Pair {
        other_id: Uuid,
        score: f64,
    }

    let coupling: Vec<Pair> = sqlx::query_as(
        r#"
        WITH refs_a AS (
            SELECT cited_work_id AS ref_id FROM citations
            WHERE citing_work_id = $1 AND cited_work_id IS NOT NULL
        ),
        sizes AS (
            SELECT citing_work_id AS work_id, COUNT(*)::float AS n
            FROM citations
            WHERE cited_work_id IS NOT NULL
            GROUP BY citing_work_id
        )
        SELECT c.citing_work_id AS other_id,
               COUNT(*)::float / NULLIF(SQRT(sa.n * sb.n), 0) AS score
        FROM citations c
        JOIN refs_a ra ON ra.ref_id = c.cited_work_id
        JOIN sizes sa ON sa.work_id = $1
        JOIN sizes sb ON sb.work_id = c.citing_work_id
        WHERE c.citing_work_id <> $1 AND c.cited_work_id IS NOT NULL
        GROUP BY c.citing_work_id, sa.n, sb.n
        ORDER BY score DESC
        LIMIT 32
        "#,
    )
    .bind(wid)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    // co-citation: shared incoming citations
    let cocite: Vec<Pair> = sqlx::query_as(
        r#"
        WITH cited_by_a AS (
            SELECT citing_work_id AS citer FROM citations
            WHERE cited_work_id = $1
        ),
        sizes AS (
            SELECT cited_work_id AS work_id, COUNT(*)::float AS n
            FROM citations
            WHERE cited_work_id IS NOT NULL
            GROUP BY cited_work_id
        )
        SELECT c.cited_work_id AS other_id,
               COUNT(*)::float / NULLIF(SQRT(sa.n * sb.n), 0) AS score
        FROM citations c
        JOIN cited_by_a ca ON ca.citer = c.citing_work_id
        JOIN sizes sa ON sa.work_id = $1
        JOIN sizes sb ON sb.work_id = c.cited_work_id
        WHERE c.cited_work_id IS NOT NULL AND c.cited_work_id <> $1
        GROUP BY c.cited_work_id, sa.n, sb.n
        ORDER BY score DESC
        LIMIT 32
        "#,
    )
    .bind(wid)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    // Merge taking max score
    let mut map: std::collections::HashMap<Uuid, f64> = std::collections::HashMap::new();
    for p in coupling.into_iter().chain(cocite) {
        map.entry(p.other_id)
            .and_modify(|s| *s = s.max(p.score))
            .or_insert(p.score);
    }
    let mut pairs: Vec<_> = map.into_iter().collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    pairs.truncate(32);

    for (other, score) in &pairs {
        graph::upsert_neighbor(
            &ctx.pool,
            NeighborDimension::CitationCoupling,
            wid,
            *other,
            *score,
        )
        .await?;
        // symmetric
        graph::upsert_neighbor(
            &ctx.pool,
            NeighborDimension::CitationCoupling,
            *other,
            wid,
            *score,
        )
        .await?;
    }
    graph::trim_neighbors(&ctx.pool, NeighborDimension::CitationCoupling, wid, 32).await?;
    tracing::info!(%wid, n = pairs.len(), "citation_coupling updated");
    Ok(())
}

/// Method lineage BFS ≤ 4 hops from works involved in a relation change.
pub async fn update_lineage(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    // Collect seed work ids from relation members if relation_id present
    let seeds: Vec<Uuid> = if let Some(rid) = job
        .payload
        .get("relation_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        sqlx::query_scalar(
            r#"
            SELECT DISTINCT anchor_work_id FROM relation_members
            WHERE relation_id = $1 AND anchor_work_id IS NOT NULL
            "#,
        )
        .bind(rid)
        .fetch_all(&ctx.pool)
        .await?
    } else if let Some(wid) = work_id(job) {
        vec![wid]
    } else {
        // full recompute for all works that have method relations
        sqlx::query_scalar(
            r#"
            SELECT DISTINCT rm.anchor_work_id
            FROM relation_members rm
            JOIN relations r ON r.id = rm.relation_id
            WHERE rm.anchor_work_id IS NOT NULL
              AND r.type IN ('uses_method_from', 'improves_on', 'alternative_to')
              AND r.review_status <> 'rejected'
            "#,
        )
        .fetch_all(&ctx.pool)
        .await?
    };

    // Build adjacency: work -> [(neighbor, edge_len)]
    #[derive(sqlx::FromRow)]
    struct Edge {
        a: Uuid,
        b: Uuid,
        edge_len: f64,
    }
    let edges: Vec<Edge> = sqlx::query_as(
        r#"
        SELECT
            src.anchor_work_id AS a,
            tgt.anchor_work_id AS b,
            CASE WHEN r.review_status = 'confirmed' THEN 1.0 ELSE 2.0 END AS edge_len
        FROM relations r
        JOIN relation_members src ON src.relation_id = r.id AND src.role = 'source'
        JOIN relation_members tgt ON tgt.relation_id = r.id AND tgt.role = 'target'
        WHERE r.type IN ('uses_method_from', 'improves_on', 'alternative_to')
          AND r.review_status <> 'rejected'
          AND src.anchor_work_id IS NOT NULL
          AND tgt.anchor_work_id IS NOT NULL
          AND src.anchor_work_id <> tgt.anchor_work_id
        "#,
    )
    .fetch_all(&ctx.pool)
    .await?;

    let mut adj: std::collections::HashMap<Uuid, Vec<(Uuid, f64)>> =
        std::collections::HashMap::new();
    for e in edges {
        adj.entry(e.a).or_default().push((e.b, e.edge_len));
        adj.entry(e.b).or_default().push((e.a, e.edge_len));
    }

    for seed in seeds {
        let scores = bfs_scores(&adj, seed, 4);
        let mut pairs: Vec<_> = scores.into_iter().collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs.truncate(32);
        for (other, score) in &pairs {
            graph::upsert_neighbor(
                &ctx.pool,
                NeighborDimension::MethodLineage,
                seed,
                *other,
                *score,
            )
            .await?;
        }
        graph::trim_neighbors(&ctx.pool, NeighborDimension::MethodLineage, seed, 32).await?;
    }
    tracing::info!("method_lineage updated");
    Ok(())
}

fn bfs_scores(
    adj: &std::collections::HashMap<Uuid, Vec<(Uuid, f64)>>,
    start: Uuid,
    max_hops: usize,
) -> std::collections::HashMap<Uuid, f64> {
    use std::collections::{HashMap, VecDeque};
    let mut dist: HashMap<Uuid, f64> = HashMap::new();
    let mut q = VecDeque::new();
    q.push_back((start, 0.0f64, 0usize));
    dist.insert(start, 0.0);

    while let Some((node, d, hops)) = q.pop_front() {
        if hops >= max_hops {
            continue;
        }
        if let Some(neis) = adj.get(&node) {
            for (n, elen) in neis {
                let nd = d + elen;
                if dist.get(n).map(|od| nd < *od).unwrap_or(true) {
                    dist.insert(*n, nd);
                    q.push_back((*n, nd, hops + 1));
                }
            }
        }
    }
    dist.remove(&start);
    dist.into_iter()
        .map(|(id, d)| (id, 1.0 / d.max(1.0)))
        .collect()
}
