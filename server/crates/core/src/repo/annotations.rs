use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{Annotation, AnnotationKind, Visibility};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct NewAnnotation {
    pub work_id: Uuid,
    pub version_id: Option<Uuid>,
    pub user_id: Uuid,
    pub kind: AnnotationKind,
    pub visibility: Visibility,
    pub anchor: Option<serde_json::Value>,
    pub body: String,
    pub parent_id: Option<Uuid>,
}

pub async fn create(pool: &PgPool, na: NewAnnotation) -> AppResult<Annotation> {
    let row = sqlx::query_as::<_, Annotation>(
        r#"
        INSERT INTO annotations (
            work_id, version_id, user_id, kind, visibility, anchor, body, parent_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, work_id, version_id, user_id, kind, visibility,
                  anchor, body, parent_id, resolved, created_at
        "#,
    )
    .bind(na.work_id)
    .bind(na.version_id)
    .bind(na.user_id)
    .bind(na.kind)
    .bind(na.visibility)
    .bind(na.anchor)
    .bind(na.body)
    .bind(na.parent_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_for_work(
    pool: &PgPool,
    work_id: Uuid,
    viewer_id: Uuid,
) -> AppResult<Vec<Annotation>> {
    let rows = sqlx::query_as::<_, Annotation>(
        r#"
        SELECT id, work_id, version_id, user_id, kind, visibility,
               anchor, body, parent_id, resolved, created_at
        FROM annotations
        WHERE work_id = $1 AND (visibility = 'team' OR user_id = $2)
        ORDER BY created_at
        "#,
    )
    .bind(work_id)
    .bind(viewer_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<Annotation> {
    sqlx::query_as::<_, Annotation>(
        r#"
        SELECT id, work_id, version_id, user_id, kind, visibility,
               anchor, body, parent_id, resolved, created_at
        FROM annotations WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("annotation {id}")))
}

/// Author-only delete (same ownership rule as node comments).
pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM annotations WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("annotation {id}")));
    }
    Ok(())
}
