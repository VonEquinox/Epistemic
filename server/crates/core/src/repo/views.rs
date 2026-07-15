//! Named distance-weight "views" (DEV.md §5 / PRD §5.6).

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub weights: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list(pool: &PgPool) -> AppResult<Vec<SavedView>> {
    Ok(sqlx::query_as::<_, SavedView>(
        r#"
        SELECT id, name, weights, created_by, created_at
        FROM saved_views
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    weights: serde_json::Value,
    created_by: Uuid,
) -> AppResult<SavedView> {
    Ok(sqlx::query_as::<_, SavedView>(
        r#"
        INSERT INTO saved_views (name, weights, created_by)
        VALUES ($1, $2, $3)
        RETURNING id, name, weights, created_by, created_at
        "#,
    )
    .bind(name)
    .bind(weights)
    .bind(created_by)
    .fetch_one(pool)
    .await?)
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<SavedView> {
    sqlx::query_as::<_, SavedView>(
        r#"
        SELECT id, name, weights, created_by, created_at
        FROM saved_views WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("view {id}")))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let r = sqlx::query(r#"DELETE FROM saved_views WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("view {id}")));
    }
    Ok(())
}
