//! Multi-aspect embeddings: one vector per fixed DNA layer + aspect neighbors.

use super::{version_id, work_id, JobContext};
use epistemic_core::domain::{AspectDef, NeighborDimension, ASPECTS, Job};
use epistemic_core::repo::{aspects, graph, works};
use epistemic_llm::vector_literal;
use uuid::Uuid;

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

    let rows = aspects::list_for_work(&ctx.pool, wid).await?;
    if rows.is_empty() {
        tracing::warn!(%wid, "embed: no paper_aspects; falling back to title/abstract/methods");
        embed_legacy_fallback(ctx, embedder, wid).await?;
        return Ok(());
    }

    let model = embedder.model().to_string();
    let mut embedded = 0u32;

    for def in ASPECTS {
        let Some(row) = rows.iter().find(|r| r.aspect == def.key) else {
            continue;
        };
        let text = aspect_embed_text(&row.summary, &row.bullets);
        if text.trim().is_empty() {
            tracing::debug!(%wid, aspect = def.key, "skip empty aspect for embed");
            graph::trim_neighbors(&ctx.pool, def.dimension, wid, 0).await?;
            continue;
        }

        let vec = embedder.embed_one(&text).await?;
        if let Some(expected) = embedder.dimensions() {
            if vec.len() as u32 != expected {
                tracing::warn!(
                    got = vec.len(),
                    expected,
                    aspect = def.key,
                    "embedding dim mismatch (continuing)"
                );
            }
        }

        let field = def.embedding_field();
        let lit = vector_literal(&vec);
        sqlx::query(
            r#"
            INSERT INTO embeddings (entity_kind, entity_id, field, model, vec)
            VALUES ('work', $1, $2, $3, $4::vector)
            ON CONFLICT (entity_kind, entity_id, field)
            DO UPDATE SET model = EXCLUDED.model, vec = EXCLUDED.vec, created_at = now()
            "#,
        )
        .bind(wid)
        .bind(&field)
        .bind(&model)
        .bind(&lit)
        .execute(&ctx.pool)
        .await?;

        update_neighbors_for_field(ctx, wid, &field, def.dimension, 32).await?;
        embedded += 1;
    }

    tracing::info!(%wid, embedded, model = %model, "multi-aspect embed done");
    Ok(())
}

fn aspect_embed_text(summary: &str, bullets: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    let s = summary.trim();
    if !s.is_empty() {
        parts.push(s.to_string());
    }
    if let Some(arr) = bullets.as_array() {
        let b = arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("; ");
        if !b.is_empty() {
            parts.push(b);
        }
    }
    parts.join("\n")
}

/// Cosine neighbors via pgvector `<=>`. score = 1 - distance.
async fn update_neighbors_for_field(
    ctx: &JobContext,
    wid: Uuid,
    field: &str,
    dimension: NeighborDimension,
    k: i64,
) -> anyhow::Result<()> {
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
        LIMIT $3
        "#,
    )
    .bind(wid)
    .bind(field)
    .bind(k)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    for r in &rows {
        graph::upsert_neighbor(&ctx.pool, dimension, wid, r.other_id, r.score).await?;
        graph::upsert_neighbor(&ctx.pool, dimension, r.other_id, wid, r.score).await?;
    }
    graph::trim_neighbors(&ctx.pool, dimension, wid, k).await?;
    Ok(())
}

async fn embed_legacy_fallback(
    ctx: &JobContext,
    embedder: &epistemic_llm::EmbeddingClient,
    wid: Uuid,
) -> anyhow::Result<()> {
    let work = works::get_work(&ctx.pool, wid).await?;
    let Some(vid) = work.primary_version_id else {
        tracing::warn!(%wid, "embed fallback: no primary version");
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
        return Ok(());
    }
    let vec = embedder.embed_one(&text).await?;
    let field = AspectDef::by_key("methods")
        .map(|d| d.embedding_field())
        .unwrap_or_else(|| "aspect:methods".into());
    let lit = vector_literal(&vec);
    sqlx::query(
        r#"
        INSERT INTO embeddings (entity_kind, entity_id, field, model, vec)
        VALUES ('work', $1, $2, $3, $4::vector)
        ON CONFLICT (entity_kind, entity_id, field)
        DO UPDATE SET model = EXCLUDED.model, vec = EXCLUDED.vec, created_at = now()
        "#,
    )
    .bind(wid)
    .bind(&field)
    .bind(embedder.model())
    .bind(&lit)
    .execute(&ctx.pool)
    .await?;
    update_neighbors_for_field(ctx, wid, &field, NeighborDimension::AspectMethods, 32).await?;
    Ok(())
}
