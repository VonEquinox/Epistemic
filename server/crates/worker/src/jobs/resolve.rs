use super::{version_id, JobContext};
use crate::metadata;
use epistemic_core::domain::Job;
use epistemic_core::repo::works;

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;

    if let Some(ref arxiv) = version.arxiv_id {
        tracing::info!(%arxiv, "resolving arXiv metadata");
        if let Some(meta) = metadata::fetch_arxiv(&ctx.http, arxiv).await? {
            works::update_version_metadata(
                &ctx.pool,
                vid,
                Some(&meta.title),
                Some(&meta.abstract_text),
                meta.year,
                meta.venue_name.as_deref(),
                meta.doi.as_deref(),
                Some("arxiv_api"),
            )
            .await?;
            // authors: re-add if empty
            let existing = works::authors_for_version(&ctx.pool, vid).await?;
            if existing.is_empty() && !meta.authors.is_empty() {
                // simple path: recreate via SQL insert of authors
                for (pos, name) in meta.authors.iter().enumerate() {
                    let author_id: uuid::Uuid = sqlx::query_scalar(
                        r#"
                        INSERT INTO authors (full_name)
                        VALUES ($1)
                        ON CONFLICT DO NOTHING
                        RETURNING id
                        "#,
                    )
                    .bind(name)
                    .fetch_optional(&ctx.pool)
                    .await?
                    .unwrap_or(
                        sqlx::query_scalar(r#"SELECT id FROM authors WHERE full_name = $1 LIMIT 1"#)
                            .bind(name)
                            .fetch_one(&ctx.pool)
                            .await?,
                    );
                    sqlx::query(
                        r#"
                        INSERT INTO version_authors (version_id, author_id, position)
                        VALUES ($1, $2, $3) ON CONFLICT DO NOTHING
                        "#,
                    )
                    .bind(vid)
                    .bind(author_id)
                    .bind(pos as i32)
                    .execute(&ctx.pool)
                    .await?;
                }
            }
            return Ok(());
        }
    }

    // Semantic Scholar fallback by arxiv or DOI
    if let Some(meta) = metadata::fetch_s2(&ctx.http, ctx.s2_api_key.as_deref(), &version).await? {
        works::update_version_metadata(
            &ctx.pool,
            vid,
            Some(&meta.title),
            Some(&meta.abstract_text),
            meta.year,
            meta.venue_name.as_deref(),
            meta.doi.as_deref(),
            Some("semantic_scholar"),
        )
        .await?;
    } else {
        tracing::warn!(%vid, "no metadata found");
    }
    Ok(())
}
