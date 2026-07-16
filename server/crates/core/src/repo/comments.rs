use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{CommentKind, NodeComment, Visibility};
use crate::error::{AppError, AppResult};

const MAX_BODY_CHARS: usize = 20_000;

#[derive(Debug, Clone)]
pub struct NewNodeComment {
    pub graph_id: Uuid,
    pub work_id: Uuid,
    pub user_id: Uuid,
    pub kind: CommentKind,
    pub visibility: Visibility,
    pub body: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct NodeCommentPatch {
    pub body: Option<String>,
    pub kind: Option<CommentKind>,
    pub visibility: Option<Visibility>,
}

fn clean_body(body: &str) -> AppResult<String> {
    let body = body.trim();
    if body.is_empty() {
        return Err(AppError::Validation("comment body is required".into()));
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(AppError::Validation(format!(
            "comment body exceeds {MAX_BODY_CHARS} characters"
        )));
    }
    Ok(body.to_string())
}

pub async fn create(pool: &PgPool, input: NewNodeComment) -> AppResult<NodeComment> {
    let body = clean_body(&input.body)?;
    let mut tx = pool.begin().await?;

    let in_graph: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM graph_works WHERE graph_id = $1 AND work_id = $2)",
    )
    .bind(input.graph_id)
    .bind(input.work_id)
    .fetch_one(&mut *tx)
    .await?;
    if !in_graph {
        return Err(AppError::Validation(format!(
            "work {} is not in graph {}",
            input.work_id, input.graph_id
        )));
    }

    if let Some(parent_id) = input.parent_id {
        #[derive(sqlx::FromRow)]
        struct ParentRow {
            graph_id: Uuid,
            work_id: Uuid,
            user_id: Uuid,
            visibility: Visibility,
        }
        let parent = sqlx::query_as::<_, ParentRow>(
            r#"
            SELECT graph_id, work_id, user_id, visibility
            FROM node_comments WHERE id = $1 FOR KEY SHARE
            "#,
        )
        .bind(parent_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comment {parent_id}")))?;
        if parent.graph_id != input.graph_id || parent.work_id != input.work_id {
            return Err(AppError::Validation(
                "reply must belong to the same graph and work".into(),
            ));
        }
        if parent.visibility == Visibility::Private && parent.user_id != input.user_id {
            return Err(AppError::NotFound(format!("comment {parent_id}")));
        }
        if parent.visibility == Visibility::Private && input.visibility != Visibility::Private {
            return Err(AppError::Validation(
                "a reply to a private comment must also be private".into(),
            ));
        }
    }

    let row = sqlx::query_as::<_, NodeComment>(
        r#"
        WITH inserted AS (
            INSERT INTO node_comments (
                graph_id, work_id, user_id, kind, visibility, body, parent_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, graph_id, work_id, user_id, kind, visibility,
                      body, parent_id, created_at, updated_at
        )
        SELECT i.id, i.graph_id, i.work_id, i.user_id, u.name AS author_name,
               i.kind, i.visibility, i.body, i.parent_id, i.created_at, i.updated_at
        FROM inserted i
        JOIN users u ON u.id = i.user_id
        "#,
    )
    .bind(input.graph_id)
    .bind(input.work_id)
    .bind(input.user_id)
    .bind(input.kind)
    .bind(input.visibility)
    .bind(body)
    .bind(input.parent_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row)
}

pub async fn list_for_node(
    pool: &PgPool,
    graph_id: Uuid,
    work_id: Uuid,
    viewer_id: Uuid,
) -> AppResult<Vec<NodeComment>> {
    Ok(sqlx::query_as::<_, NodeComment>(
        r#"
        SELECT c.id, c.graph_id, c.work_id, c.user_id, u.name AS author_name,
               c.kind, c.visibility, c.body, c.parent_id, c.created_at, c.updated_at
        FROM node_comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.graph_id = $1 AND c.work_id = $2
          AND (c.visibility = 'team' OR c.user_id = $3)
        ORDER BY c.created_at, c.id
        "#,
    )
    .bind(graph_id)
    .bind(work_id)
    .bind(viewer_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<NodeComment> {
    sqlx::query_as::<_, NodeComment>(
        r#"
        SELECT c.id, c.graph_id, c.work_id, c.user_id, u.name AS author_name,
               c.kind, c.visibility, c.body, c.parent_id, c.created_at, c.updated_at
        FROM node_comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("comment {id}")))
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    patch: NodeCommentPatch,
) -> AppResult<NodeComment> {
    if patch.body.is_none() && patch.kind.is_none() && patch.visibility.is_none() {
        return Err(AppError::Validation("comment patch is empty".into()));
    }
    let body = patch.body.as_deref().map(clean_body).transpose()?;

    let row = sqlx::query_as::<_, NodeComment>(
        r#"
        WITH updated AS (
            UPDATE node_comments
            SET body = COALESCE($3, body),
                kind = COALESCE($4, kind),
                visibility = COALESCE($5, visibility),
                updated_at = now()
            WHERE id = $1 AND user_id = $2
            RETURNING id, graph_id, work_id, user_id, kind, visibility,
                      body, parent_id, created_at, updated_at
        )
        SELECT c.id, c.graph_id, c.work_id, c.user_id, u.name AS author_name,
               c.kind, c.visibility, c.body, c.parent_id, c.created_at, c.updated_at
        FROM updated c
        JOIN users u ON u.id = c.user_id
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(body)
    .bind(patch.kind)
    .bind(patch.visibility)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("comment {id}")))?;

    Ok(row)
}

pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM node_comments WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("comment {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clean_body;

    #[test]
    fn comment_body_is_trimmed_and_required() {
        assert_eq!(clean_body("  idea  ").unwrap(), "idea");
        assert!(clean_body("   ").is_err());
    }
}
