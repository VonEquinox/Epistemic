use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::Project;
use crate::error::{AppError, AppResult};

pub async fn create_project(pool: &PgPool, name: &str, description: &str) -> AppResult<Project> {
    let p = sqlx::query_as::<_, Project>(
        r#"
        INSERT INTO projects (name, description)
        VALUES ($1, $2)
        RETURNING id, name, description, created_at
        "#,
    )
    .bind(name)
    .bind(description)
    .fetch_one(pool)
    .await?;
    Ok(p)
}

pub async fn list_projects(pool: &PgPool) -> AppResult<Vec<Project>> {
    let rows = sqlx::query_as::<_, Project>(
        r#"SELECT id, name, description, created_at FROM projects ORDER BY name"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_project(pool: &PgPool, id: Uuid) -> AppResult<Project> {
    sqlx::query_as::<_, Project>(
        r#"SELECT id, name, description, created_at FROM projects WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {id}")))
}

pub async fn attach_work(pool: &PgPool, work_id: Uuid, project_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO work_projects (work_id, project_id)
        VALUES ($1, $2) ON CONFLICT DO NOTHING
        "#,
    )
    .bind(work_id)
    .bind(project_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn projects_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<Project>> {
    let rows = sqlx::query_as::<_, Project>(
        r#"
        SELECT p.id, p.name, p.description, p.created_at
        FROM projects p
        JOIN work_projects wp ON wp.project_id = p.id
        WHERE wp.work_id = $1
        ORDER BY p.name
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct CoverageEntry {
    pub work_id: Uuid,
    pub title: String,
    pub readers: Vec<CoverageReader>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct CoverageReader {
    pub user_id: Uuid,
    pub name: String,
    pub status: String,
    pub starred: bool,
}

#[derive(sqlx::FromRow)]
struct WorkTitleRow {
    id: Uuid,
    title: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ReaderRow {
    user_id: Uuid,
    name: String,
    status: Option<String>,
    starred: bool,
}

pub async fn project_coverage(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<CoverageEntry>> {
    get_project(pool, project_id).await?;

    let works = sqlx::query_as::<_, WorkTitleRow>(
        r#"
        SELECT w.id, COALESCE(v.title, w.title_norm) AS title
        FROM works w
        JOIN work_projects wp ON wp.work_id = w.id
        LEFT JOIN versions v ON v.id = w.primary_version_id
        WHERE wp.project_id = $1
        ORDER BY w.created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(works.len());
    for w in works {
        let readers = sqlx::query_as::<_, ReaderRow>(
            r#"
            SELECT u.id AS user_id, u.name, rs.status::text AS status, rs.starred
            FROM reading_status rs
            JOIN users u ON u.id = rs.user_id
            WHERE rs.work_id = $1 AND rs.status <> 'unread'
            ORDER BY u.name
            "#,
        )
        .bind(w.id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| CoverageReader {
            user_id: r.user_id,
            name: r.name,
            status: r.status.unwrap_or_else(|| "unread".into()),
            starred: r.starred,
        })
        .collect();

        out.push(CoverageEntry {
            work_id: w.id,
            title: w.title.unwrap_or_default(),
            readers,
        });
    }
    Ok(out)
}
