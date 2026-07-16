//! paper_aspects — fixed multi-layer DNA per work.

use crate::domain::PaperAspect;
use crate::error::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn upsert(
    pool: &PgPool,
    work_id: Uuid,
    aspect: &str,
    summary: &str,
    bullets: &serde_json::Value,
    source_text: &str,
    page: i32,
    model: Option<&str>,
    prompt_version: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO paper_aspects (
            work_id, aspect, summary, bullets, source_text, page, model, prompt_version
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (work_id, aspect) DO UPDATE SET
            summary = EXCLUDED.summary,
            bullets = EXCLUDED.bullets,
            source_text = EXCLUDED.source_text,
            page = EXCLUDED.page,
            model = EXCLUDED.model,
            prompt_version = EXCLUDED.prompt_version,
            updated_at = now()
        "#,
    )
    .bind(work_id)
    .bind(aspect)
    .bind(summary)
    .bind(bullets)
    .bind(source_text)
    .bind(page)
    .bind(model)
    .bind(prompt_version)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<PaperAspect>> {
    let rows = sqlx::query_as::<_, PaperAspect>(
        r#"
        SELECT work_id, aspect, summary, bullets, source_text, page,
               model, prompt_version, created_at, updated_at
        FROM paper_aspects
        WHERE work_id = $1
        ORDER BY aspect
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<()> {
    sqlx::query(r#"DELETE FROM paper_aspects WHERE work_id = $1"#)
        .bind(work_id)
        .execute(pool)
        .await?;
    Ok(())
}
