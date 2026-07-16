//! Work-level text embedding → embeddings table + topic neighbors.
//!
//! Provider: OpenAI-compatible `/v1/embeddings` (SiliconFlow Qwen3-Embedding-8B).

use super::{version_id, work_id, JobContext};
use epistemic_core::domain::{NeighborDimension, Job};
use epistemic_core::repo::{graph, works};
use epistemic_llm::vector_literal;
use uuid::Uuid;

const FIELD: &str = "title_abstract_methods";

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let embedder = ctx
        .embed
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Embedding client not configured (EMBEDDING_API_KEY)"))?;

    let wid = if let Some(w) = work_id(job) {
        w
    } else {
        let vid = version_id(job)?;
        works::get_version(&ctx.pool, vid).await?.work_id
    };

    let work = works::get_work(&ctx.pool, wid).await?;
    let Some(vid) = work.primary_version_id else {
        tracing::warn!(%wid, "embed: no primary version");
        return Ok(());
    };
    let version = works::get_version(&ctx.pool, vid).await?;
    let methods = works::list_methods(&ctx.pool, wid)
        .await
        .unwrap_or_default();
    let methods_blob = methods
        .iter()
        .map(|m| {
            if m.description.is_empty() {
                m.name.clone()
            } else {
                format!("{}: {}", m.name, m.description)
            }
        })
        .collect::<Vec<_>>()
        .join("; ");

    let text = format!(
        "title: {}\nabstract: {}\nmethods: {}",
        version.title,
        version.abstract_text.chars().take(4000).collect::<String>(),
        methods_blob
    );
    if text.trim().is_empty() {
        tracing::warn!(%wid, "embed: empty text");
        return Ok(());
    }

    let vec = embedder.embed_one(&text).await?;
    if let Some(expected) = embedder.dimensions() {
        if vec.len() as u32 != expected {
            tracing::warn!(
                got = vec.len(),
                expected,
                "embedding dim mismatch (continuing)"
            );
        }
    }

    let lit = vector_literal(&vec);
    let model = embedder.model();

    sqlx::query(
        r#"
        INSERT INTO embeddings (entity_kind, entity_id, field, model, vec)
        VALUES ('work', $1, $2, $3, $4::vector)
        ON CONFLICT (entity_kind, entity_id, field)
        DO UPDATE SET model = EXCLUDED.model, vec = EXCLUDED.vec, created_at = now()
        "#,
    )
    .bind(wid)
    .bind(FIELD)
    .bind(model)
    .bind(&lit)
    .execute(&ctx.pool)
    .await?;

    update_topic_neighbors(ctx, wid).await?;
    tracing::info!(%wid, dim = vec.len(), model, "embed done");
    Ok(())
}

/// Cosine neighbors via pgvector `<=>` (cosine distance). score = 1 - distance.
async fn update_topic_neighbors(ctx: &JobContext, wid: Uuid) -> anyhow::Result<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        other_id: Uuid,
        score: f64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        WITH seed AS (
            SELECT vec FROM embeddings
            WHERE entity_kind = 'work' AND entity_id = $1 AND field = $2
        )
        SELECT e.entity_id AS other_id,
               (1.0 - (e.vec <=> seed.vec))::float8 AS score
        FROM embeddings e, seed
        WHERE e.entity_kind = 'work'
          AND e.field = $2
          AND e.entity_id <> $1
        ORDER BY e.vec <=> seed.vec
        LIMIT 32
        "#,
    )
    .bind(wid)
    .bind(FIELD)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    for r in &rows {
        if r.score.is_finite() {
            graph::upsert_neighbor(
                &ctx.pool,
                NeighborDimension::Topic,
                wid,
                r.other_id,
                r.score,
            )
            .await?;
            graph::upsert_neighbor(
                &ctx.pool,
                NeighborDimension::Topic,
                r.other_id,
                wid,
                r.score,
            )
            .await?;
        }
    }
    graph::trim_neighbors(&ctx.pool, NeighborDimension::Topic, wid, 32).await?;
    Ok(())
}
