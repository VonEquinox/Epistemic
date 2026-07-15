use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    Author, Claim, Method, Version, VersionAuthor, VersionKind, VersionRow, Work, WorkCard,
};
use crate::error::{AppError, AppResult};
use crate::repo::{projects, reading};
use crate::util::normalize_title;

#[derive(Debug, Clone)]
pub struct NewVersion {
    pub kind: VersionKind,
    pub arxiv_id: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub title: String,
    pub abstract_text: String,
    pub year: Option<i32>,
    pub venue_name: Option<String>,
    pub metadata_source: Option<String>,
    pub author_names: Vec<String>,
}

pub async fn create_or_get_work(
    pool: &PgPool,
    nv: NewVersion,
    created_by: Option<Uuid>,
) -> AppResult<(Work, Version, bool)> {
    if let Some(ref arxiv) = nv.arxiv_id {
        if let Some(existing) = find_version_by_arxiv(pool, arxiv).await? {
            let work = get_work(pool, existing.work_id).await?;
            return Ok((work, existing, false));
        }
    }
    if let Some(ref doi) = nv.doi {
        if let Some(existing) = find_version_by_doi(pool, doi).await? {
            let work = get_work(pool, existing.work_id).await?;
            return Ok((work, existing, false));
        }
    }
    let title_norm = normalize_title(&nv.title);
    if let Some(year) = nv.year {
        if let Some((work, ver)) = find_by_title_year(pool, &title_norm, year).await? {
            return Ok((work, ver, false));
        }
    }

    let mut tx = pool.begin().await?;

    let work = sqlx::query_as::<_, Work>(
        r#"
        INSERT INTO works (title_norm, created_by)
        VALUES ($1, $2)
        RETURNING id, title_norm, primary_version_id, created_by, created_at
        "#,
    )
    .bind(&title_norm)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await?;

    let ver_row = sqlx::query_as::<_, VersionRow>(
        r#"
        INSERT INTO versions (
            work_id, kind, arxiv_id, doi, url, title, abstract,
            year, venue_name, metadata_source
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id, work_id, kind, arxiv_id, doi, url, title,
                  abstract, year, venue_name, pdf_path, tei_path, metadata_source, created_at
        "#,
    )
    .bind(work.id)
    .bind(nv.kind)
    .bind(&nv.arxiv_id)
    .bind(&nv.doi)
    .bind(&nv.url)
    .bind(&nv.title)
    .bind(&nv.abstract_text)
    .bind(nv.year)
    .bind(&nv.venue_name)
    .bind(&nv.metadata_source)
    .fetch_one(&mut *tx)
    .await?;
    let version: Version = ver_row.into();

    sqlx::query(r#"UPDATE works SET primary_version_id = $1 WHERE id = $2"#)
        .bind(version.id)
        .bind(work.id)
        .execute(&mut *tx)
        .await?;

    for (pos, name) in nv.author_names.iter().enumerate() {
        let author_id = upsert_author_tx(&mut tx, name).await?;
        sqlx::query(
            r#"
            INSERT INTO version_authors (version_id, author_id, position)
            VALUES ($1, $2, $3) ON CONFLICT DO NOTHING
            "#,
        )
        .bind(version.id)
        .bind(author_id)
        .bind(pos as i32)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    let work = get_work(pool, work.id).await?;
    Ok((work, version, true))
}

async fn upsert_author_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    full_name: &str,
) -> AppResult<Uuid> {
    let existing: Option<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM authors WHERE full_name = $1 LIMIT 1"#)
            .bind(full_name)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id: Uuid = sqlx::query_scalar(r#"INSERT INTO authors (full_name) VALUES ($1) RETURNING id"#)
        .bind(full_name)
        .fetch_one(&mut **tx)
        .await?;
    Ok(id)
}

pub async fn get_work(pool: &PgPool, id: Uuid) -> AppResult<Work> {
    sqlx::query_as::<_, Work>(
        r#"SELECT id, title_norm, primary_version_id, created_by, created_at FROM works WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("work {id}")))
}

pub async fn get_version(pool: &PgPool, id: Uuid) -> AppResult<Version> {
    let row = sqlx::query_as::<_, VersionRow>(
        r#"
        SELECT id, work_id, kind, arxiv_id, doi, url, title, abstract, year,
               venue_name, pdf_path, tei_path, metadata_source, created_at
        FROM versions WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("version {id}")))?;
    Ok(row.into())
}

pub async fn find_version_by_arxiv(pool: &PgPool, arxiv_id: &str) -> AppResult<Option<Version>> {
    let row = sqlx::query_as::<_, VersionRow>(
        r#"
        SELECT id, work_id, kind, arxiv_id, doi, url, title, abstract, year,
               venue_name, pdf_path, tei_path, metadata_source, created_at
        FROM versions WHERE arxiv_id = $1
        "#,
    )
    .bind(arxiv_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(Into::into))
}

pub async fn find_version_by_doi(pool: &PgPool, doi: &str) -> AppResult<Option<Version>> {
    let row = sqlx::query_as::<_, VersionRow>(
        r#"
        SELECT id, work_id, kind, arxiv_id, doi, url, title, abstract, year,
               venue_name, pdf_path, tei_path, metadata_source, created_at
        FROM versions WHERE doi = $1
        "#,
    )
    .bind(doi)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(Into::into))
}

async fn find_by_title_year(
    pool: &PgPool,
    title_norm: &str,
    year: i32,
) -> AppResult<Option<(Work, Version)>> {
    let work_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT w.id FROM works w
        JOIN versions v ON v.id = w.primary_version_id
        WHERE w.title_norm = $1 AND v.year = $2 LIMIT 1
        "#,
    )
    .bind(title_norm)
    .bind(year)
    .fetch_optional(pool)
    .await?;

    if let Some(wid) = work_id {
        let work = get_work(pool, wid).await?;
        if let Some(vid) = work.primary_version_id {
            let ver = get_version(pool, vid).await?;
            return Ok(Some((work, ver)));
        }
    }
    Ok(None)
}

pub async fn list_versions_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<Version>> {
    let rows = sqlx::query_as::<_, VersionRow>(
        r#"
        SELECT id, work_id, kind, arxiv_id, doi, url, title, abstract, year,
               venue_name, pdf_path, tei_path, metadata_source, created_at
        FROM versions WHERE work_id = $1 ORDER BY created_at
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[derive(sqlx::FromRow)]
struct AuthorJoinRow {
    id: Uuid,
    full_name: String,
    s2_author_id: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    position: i32,
}

pub async fn authors_for_version(pool: &PgPool, version_id: Uuid) -> AppResult<Vec<VersionAuthor>> {
    let rows = sqlx::query_as::<_, AuthorJoinRow>(
        r#"
        SELECT a.id, a.full_name, a.s2_author_id, a.created_at, va.position
        FROM authors a
        JOIN version_authors va ON va.author_id = a.id
        WHERE va.version_id = $1
        ORDER BY va.position
        "#,
    )
    .bind(version_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| VersionAuthor {
            author: Author {
                id: r.id,
                full_name: r.full_name,
                s2_author_id: r.s2_author_id,
                created_at: r.created_at,
            },
            position: r.position,
        })
        .collect())
}

#[derive(Debug, Clone, Default)]
pub struct WorkListQuery {
    pub query: Option<String>,
    pub project_id: Option<Uuid>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct WorkListItem {
    pub work: Work,
    pub title: String,
    pub year: Option<i32>,
    pub arxiv_id: Option<String>,
    pub venue_name: Option<String>,
    pub authors: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct WorkListRow {
    id: Uuid,
    title_norm: String,
    primary_version_id: Option<Uuid>,
    created_by: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    v_title: Option<String>,
    v_year: Option<i32>,
    v_arxiv: Option<String>,
    v_venue: Option<String>,
}

pub async fn list_works(pool: &PgPool, q: WorkListQuery) -> AppResult<Vec<WorkListItem>> {
    let limit = if q.limit <= 0 { 50 } else { q.limit.min(200) };
    let offset = q.offset.max(0);

    let sql = match (&q.project_id, &q.query) {
        (Some(_), Some(_)) => r#"
            SELECT w.id, w.title_norm, w.primary_version_id, w.created_by, w.created_at,
                   v.title as v_title, v.year as v_year, v.arxiv_id as v_arxiv, v.venue_name as v_venue
            FROM works w
            JOIN work_projects wp ON wp.work_id = w.id AND wp.project_id = $1
            LEFT JOIN versions v ON v.id = w.primary_version_id
            WHERE w.title_norm ILIKE $2 OR v.title ILIKE $2 OR v.arxiv_id ILIKE $2
            ORDER BY w.created_at DESC LIMIT $3 OFFSET $4
        "#,
        (Some(_), None) => r#"
            SELECT w.id, w.title_norm, w.primary_version_id, w.created_by, w.created_at,
                   v.title as v_title, v.year as v_year, v.arxiv_id as v_arxiv, v.venue_name as v_venue
            FROM works w
            JOIN work_projects wp ON wp.work_id = w.id AND wp.project_id = $1
            LEFT JOIN versions v ON v.id = w.primary_version_id
            ORDER BY w.created_at DESC LIMIT $2 OFFSET $3
        "#,
        (None, Some(_)) => r#"
            SELECT w.id, w.title_norm, w.primary_version_id, w.created_by, w.created_at,
                   v.title as v_title, v.year as v_year, v.arxiv_id as v_arxiv, v.venue_name as v_venue
            FROM works w
            LEFT JOIN versions v ON v.id = w.primary_version_id
            WHERE w.title_norm ILIKE $1 OR v.title ILIKE $1 OR v.arxiv_id ILIKE $1
            ORDER BY w.created_at DESC LIMIT $2 OFFSET $3
        "#,
        (None, None) => r#"
            SELECT w.id, w.title_norm, w.primary_version_id, w.created_by, w.created_at,
                   v.title as v_title, v.year as v_year, v.arxiv_id as v_arxiv, v.venue_name as v_venue
            FROM works w
            LEFT JOIN versions v ON v.id = w.primary_version_id
            ORDER BY w.created_at DESC LIMIT $1 OFFSET $2
        "#,
    };

    let mut query = sqlx::query_as::<_, WorkListRow>(sql);
    match (&q.project_id, &q.query) {
        (Some(pid), Some(text)) => {
            query = query
                .bind(*pid)
                .bind(format!("%{text}%"))
                .bind(limit)
                .bind(offset);
        }
        (Some(pid), None) => {
            query = query.bind(*pid).bind(limit).bind(offset);
        }
        (None, Some(text)) => {
            query = query
                .bind(format!("%{text}%"))
                .bind(limit)
                .bind(offset);
        }
        (None, None) => {
            query = query.bind(limit).bind(offset);
        }
    }

    let rows = query.fetch_all(pool).await?;
    let mut items = Vec::with_capacity(rows.len());
    for r in rows {
        let work = Work {
            id: r.id,
            title_norm: r.title_norm.clone(),
            primary_version_id: r.primary_version_id,
            created_by: r.created_by,
            created_at: r.created_at,
        };
        let authors = if let Some(vid) = r.primary_version_id {
            authors_for_version(pool, vid)
                .await?
                .into_iter()
                .map(|a| a.author.full_name)
                .collect()
        } else {
            vec![]
        };
        items.push(WorkListItem {
            title: r.v_title.unwrap_or(r.title_norm),
            year: r.v_year,
            arxiv_id: r.v_arxiv,
            venue_name: r.v_venue,
            authors,
            work,
        });
    }
    Ok(items)
}

pub async fn get_work_card(pool: &PgPool, work_id: Uuid) -> AppResult<WorkCard> {
    let work = get_work(pool, work_id).await?;
    let versions = list_versions_for_work(pool, work_id).await?;
    let primary_version = if let Some(vid) = work.primary_version_id {
        Some(get_version(pool, vid).await?)
    } else {
        versions.first().cloned()
    };
    let authors = if let Some(ref v) = primary_version {
        authors_for_version(pool, v.id).await?
    } else {
        vec![]
    };
    let projs = projects::projects_for_work(pool, work_id).await?;
    let claims = list_claims(pool, work_id).await?;
    let methods = list_methods(pool, work_id).await?;
    let reading_rows = reading::list_for_work(pool, work_id).await?;
    let annotations_count: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*) FROM annotations WHERE work_id = $1"#)
            .bind(work_id)
            .fetch_one(pool)
            .await?;

    Ok(WorkCard {
        work,
        primary_version,
        versions,
        authors,
        projects: projs,
        claims,
        methods,
        reading: reading_rows,
        annotations_count,
    })
}

pub async fn list_claims(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<Claim>> {
    let rows = sqlx::query_as::<_, Claim>(
        r#"
        SELECT id, work_id, text, source, review_status, created_by, model_version, created_at
        FROM claims WHERE work_id = $1 ORDER BY created_at
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_methods(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<Method>> {
    let rows = sqlx::query_as::<_, Method>(
        r#"
        SELECT id, work_id, parent_id, name, description, source, review_status,
               created_by, model_version, created_at
        FROM methods WHERE work_id = $1 ORDER BY created_at
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_version_paths(
    pool: &PgPool,
    version_id: Uuid,
    pdf_path: Option<&str>,
    tei_path: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE versions
        SET pdf_path = COALESCE($2, pdf_path), tei_path = COALESCE($3, tei_path)
        WHERE id = $1
        "#,
    )
    .bind(version_id)
    .bind(pdf_path)
    .bind(tei_path)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_version_metadata(
    pool: &PgPool,
    version_id: Uuid,
    title: Option<&str>,
    abstract_text: Option<&str>,
    year: Option<i32>,
    venue_name: Option<&str>,
    doi: Option<&str>,
    metadata_source: Option<&str>,
) -> AppResult<Version> {
    let row = sqlx::query_as::<_, VersionRow>(
        r#"
        UPDATE versions SET
            title = COALESCE($2, title),
            abstract = COALESCE($3, abstract),
            year = COALESCE($4, year),
            venue_name = COALESCE($5, venue_name),
            doi = COALESCE($6, doi),
            metadata_source = COALESCE($7, metadata_source)
        WHERE id = $1
        RETURNING id, work_id, kind, arxiv_id, doi, url, title, abstract, year,
                  venue_name, pdf_path, tei_path, metadata_source, created_at
        "#,
    )
    .bind(version_id)
    .bind(title)
    .bind(abstract_text)
    .bind(year)
    .bind(venue_name)
    .bind(doi)
    .bind(metadata_source)
    .fetch_one(pool)
    .await?;

    if let Some(t) = title {
        let norm = normalize_title(t);
        sqlx::query(r#"UPDATE works SET title_norm = $1 WHERE id = $2"#)
            .bind(norm)
            .bind(row.work_id)
            .execute(pool)
            .await?;
    }
    Ok(row.into())
}

pub async fn merge_works(
    pool: &PgPool,
    kept_work_id: Uuid,
    merged_work_id: Uuid,
    merged_by: Uuid,
) -> AppResult<Work> {
    if kept_work_id == merged_work_id {
        return Err(AppError::BadRequest("cannot merge work with itself".into()));
    }
    let kept = get_work(pool, kept_work_id).await?;
    let merged = get_work(pool, merged_work_id).await?;
    let snapshot = serde_json::json!({
        "merged_work": merged,
        "versions": list_versions_for_work(pool, merged_work_id).await?,
    });

    let mut tx = pool.begin().await?;
    for (sql, a, b) in [
        (
            "UPDATE versions SET work_id = $1 WHERE work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
        (
            "UPDATE claims SET work_id = $1 WHERE work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
        (
            "UPDATE methods SET work_id = $1 WHERE work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
        (
            "UPDATE annotations SET work_id = $1 WHERE work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
        (
            "UPDATE citations SET citing_work_id = $1 WHERE citing_work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
        (
            "UPDATE citations SET cited_work_id = $1 WHERE cited_work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
        (
            "UPDATE relation_members SET anchor_work_id = $1 WHERE anchor_work_id = $2",
            kept_work_id,
            merged_work_id,
        ),
    ] {
        sqlx::query(sql)
            .bind(a)
            .bind(b)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO work_projects (work_id, project_id)
        SELECT $1, project_id FROM work_projects WHERE work_id = $2
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(kept_work_id)
    .bind(merged_work_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(r#"DELETE FROM work_projects WHERE work_id = $1"#)
        .bind(merged_work_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"
        UPDATE relation_members SET entity_id = $1
        WHERE entity_kind = 'work' AND entity_id = $2
        "#,
    )
    .bind(kept_work_id)
    .bind(merged_work_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO merge_history (kept_work_id, merged_work_id, snapshot, merged_by)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(kept_work_id)
    .bind(merged_work_id)
    .bind(snapshot)
    .bind(merged_by)
    .execute(&mut *tx)
    .await?;

    sqlx::query(r#"DELETE FROM reading_status WHERE work_id = $1"#)
        .bind(merged_work_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM works WHERE id = $1"#)
        .bind(merged_work_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(kept)
}
