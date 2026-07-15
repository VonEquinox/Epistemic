use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{ReadingLevel, ReadingStatusRow};
use crate::error::AppResult;

pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    work_id: Uuid,
    status: ReadingLevel,
    starred: bool,
) -> AppResult<ReadingStatusRow> {
    let row = sqlx::query_as::<_, ReadingStatusRow>(
        r#"
        INSERT INTO reading_status (user_id, work_id, status, starred, updated_at)
        VALUES ($1, $2, $3, $4, now())
        ON CONFLICT (user_id, work_id) DO UPDATE
        SET status = EXCLUDED.status, starred = EXCLUDED.starred, updated_at = now()
        RETURNING user_id, work_id, status, starred, updated_at
        "#,
    )
    .bind(user_id)
    .bind(work_id)
    .bind(status)
    .bind(starred)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get(
    pool: &PgPool,
    user_id: Uuid,
    work_id: Uuid,
) -> AppResult<Option<ReadingStatusRow>> {
    let row = sqlx::query_as::<_, ReadingStatusRow>(
        r#"
        SELECT user_id, work_id, status, starred, updated_at
        FROM reading_status WHERE user_id = $1 AND work_id = $2
        "#,
    )
    .bind(user_id)
    .bind(work_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<ReadingStatusRow>> {
    let rows = sqlx::query_as::<_, ReadingStatusRow>(
        r#"
        SELECT user_id, work_id, status, starred, updated_at
        FROM reading_status WHERE work_id = $1 ORDER BY updated_at DESC
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<ReadingStatusRow>> {
    let rows = sqlx::query_as::<_, ReadingStatusRow>(
        r#"
        SELECT user_id, work_id, status, starred, updated_at
        FROM reading_status WHERE user_id = $1 ORDER BY updated_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
