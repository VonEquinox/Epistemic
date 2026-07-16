use sqlx::PgPool;
use uuid::Uuid;

use serde::{Deserialize, Serialize};

use crate::domain::{
    Author, Claim, Method, ReadingStatusRow, Version, VersionAuthor, VersionKind, VersionRow, Work,
    WorkCard,
};
use crate::error::{AppError, AppResult};
use crate::repo::{jobs, projects, reading, relations};
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
    let aspects = crate::repo::aspects::list_for_work(pool, work_id)
        .await
        .unwrap_or_default();
    let reading_rows = reading::list_for_work(pool, work_id).await?;
    let annotations_count: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*) FROM annotations WHERE work_id = $1"#)
            .bind(work_id)
            .fetch_one(pool)
            .await?;
    let evidence = crate::repo::evidence::list_for_work(pool, work_id).await?;
    let relations = relations::list_for_work(pool, work_id).await?;
    let pipeline = jobs::jobs_for_work(pool, work_id).await?;

    Ok(WorkCard {
        work,
        primary_version,
        versions,
        authors,
        projects: projs,
        claims,
        methods,
        aspects,
        reading: reading_rows,
        annotations_count,
        evidence,
        relations,
        pipeline,
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

/// Snapshot of a merged work, stored in `merge_history.snapshot` for reversible split.
///
/// Best-effort restore limitations (see `split_work`):
/// - Neighbors folded onto the kept work at merge time stay on kept; only snapshotted
///   pre-merge neighbor edges involving the merged work are re-inserted.
/// - If a relation_member PK already occupies `(relation, work, role)` for the kept work,
///   the merged work-entity member may be dropped at merge and re-inserted on split if possible.
/// - Reading status is restored with ON CONFLICT DO NOTHING (kept-work status wins).
/// - Claims/methods/annotations are moved back only by snapshotted ids; DNA created on the
///   kept work after the merge is not reassigned.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeSnapshot {
    merged_work: Work,
    versions: Vec<Version>,
    project_ids: Vec<Uuid>,
    reading_status: Vec<ReadingStatusRow>,
    claim_ids: Vec<Uuid>,
    method_ids: Vec<Uuid>,
    annotation_ids: Vec<Uuid>,
    citation_ids_citing: Vec<Uuid>,
    citation_ids_cited: Vec<Uuid>,
    /// Pre-merge relation_members rows that referenced the merged work as entity or anchor.
    relation_members: Vec<MergeRelationMember>,
    neighbors: Vec<MergeNeighbor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
struct MergeRelationMember {
    relation_id: Uuid,
    entity_kind: String,
    entity_id: Uuid,
    role: String,
    anchor_work_id: Option<Uuid>,
    position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
struct MergeNeighbor {
    dimension: String,
    work_id: Uuid,
    neighbor_work_id: Uuid,
    score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
struct MergeHistoryRow {
    id: Uuid,
    kept_work_id: Uuid,
    merged_work_id: Uuid,
    snapshot: serde_json::Value,
    merged_by: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    reverted_at: Option<chrono::DateTime<chrono::Utc>>,
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

    let versions = list_versions_for_work(pool, merged_work_id).await?;
    let project_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT project_id FROM work_projects WHERE work_id = $1"#,
    )
    .bind(merged_work_id)
    .fetch_all(pool)
    .await?;
    let reading_status = reading::list_for_work(pool, merged_work_id).await?;
    let claim_ids: Vec<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM claims WHERE work_id = $1"#)
            .bind(merged_work_id)
            .fetch_all(pool)
            .await?;
    let method_ids: Vec<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM methods WHERE work_id = $1"#)
            .bind(merged_work_id)
            .fetch_all(pool)
            .await?;
    let annotation_ids: Vec<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM annotations WHERE work_id = $1"#)
            .bind(merged_work_id)
            .fetch_all(pool)
            .await?;
    let citation_ids_citing: Vec<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM citations WHERE citing_work_id = $1"#)
            .bind(merged_work_id)
            .fetch_all(pool)
            .await?;
    let citation_ids_cited: Vec<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM citations WHERE cited_work_id = $1"#)
            .bind(merged_work_id)
            .fetch_all(pool)
            .await?;
    let relation_members = sqlx::query_as::<_, MergeRelationMember>(
        r#"
        SELECT relation_id, entity_kind::text AS entity_kind, entity_id,
               role::text AS role, anchor_work_id, position
        FROM relation_members
        WHERE anchor_work_id = $1
           OR (entity_kind = 'work' AND entity_id = $1)
        "#,
    )
    .bind(merged_work_id)
    .fetch_all(pool)
    .await?;
    let neighbors = sqlx::query_as::<_, MergeNeighbor>(
        r#"
        SELECT dimension::text AS dimension, work_id, neighbor_work_id, score
        FROM neighbors
        WHERE work_id = $1 OR neighbor_work_id = $1
        "#,
    )
    .bind(merged_work_id)
    .fetch_all(pool)
    .await?;

    let snapshot = MergeSnapshot {
        merged_work: merged,
        versions,
        project_ids,
        reading_status,
        claim_ids,
        method_ids,
        annotation_ids,
        citation_ids_citing,
        citation_ids_cited,
        relation_members,
        neighbors,
    };
    let snapshot_json = serde_json::to_value(&snapshot)
        .map_err(|e| AppError::Other(anyhow::anyhow!("serialize merge snapshot: {e}")))?;

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

    // Work-entity relation members: rewrite entity_id; drop if PK already occupied by kept.
    sqlx::query(
        r#"
        DELETE FROM relation_members rm
        WHERE rm.entity_kind = 'work' AND rm.entity_id = $2
          AND EXISTS (
            SELECT 1 FROM relation_members k
            WHERE k.relation_id = rm.relation_id
              AND k.entity_kind = 'work'
              AND k.entity_id = $1
              AND k.role = rm.role
          )
        "#,
    )
    .bind(kept_work_id)
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

    // Fold neighbors onto kept (avoid self-loops / PK conflicts), then drop remainder.
    sqlx::query(
        r#"
        INSERT INTO neighbors (dimension, work_id, neighbor_work_id, score)
        SELECT dimension, $1, neighbor_work_id, score
        FROM neighbors
        WHERE work_id = $2 AND neighbor_work_id <> $1
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(kept_work_id)
    .bind(merged_work_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO neighbors (dimension, work_id, neighbor_work_id, score)
        SELECT dimension, work_id, $1, score
        FROM neighbors
        WHERE neighbor_work_id = $2 AND work_id <> $1
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(kept_work_id)
    .bind(merged_work_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(r#"DELETE FROM neighbors WHERE work_id = $1 OR neighbor_work_id = $1"#)
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
    .bind(&snapshot_json)
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

/// Reverse a prior merge: restore the merged work from `merge_history.snapshot`.
///
/// `kept_work_id` is the work that absorbed the merge (path param). Identify the
/// history row via `merge_history_id` and/or `merged_work_id` (at least one required).
pub async fn split_work(
    pool: &PgPool,
    kept_work_id: Uuid,
    merge_history_id: Option<Uuid>,
    merged_work_id: Option<Uuid>,
) -> AppResult<Work> {
    if merge_history_id.is_none() && merged_work_id.is_none() {
        return Err(AppError::BadRequest(
            "provide merge_history_id or merged_work_id".into(),
        ));
    }

    let hist = load_merge_history(pool, kept_work_id, merge_history_id, merged_work_id).await?;
    if hist.reverted_at.is_some() {
        return Err(AppError::Conflict(format!(
            "merge_history {} already reverted",
            hist.id
        )));
    }
    if hist.kept_work_id != kept_work_id {
        return Err(AppError::BadRequest(format!(
            "merge_history {} kept_work_id does not match path work",
            hist.id
        )));
    }

    // Ensure kept still exists.
    let _kept = get_work(pool, kept_work_id).await?;

    let snap: MergeSnapshot = serde_json::from_value(hist.snapshot.clone()).map_err(|e| {
        AppError::Other(anyhow::anyhow!(
            "invalid merge snapshot for history {}: {e}",
            hist.id
        ))
    })?;

    let restored_id = snap.merged_work.id;
    if restored_id != hist.merged_work_id {
        return Err(AppError::Other(anyhow::anyhow!(
            "snapshot merged_work.id mismatch with merge_history.merged_work_id"
        )));
    }

    // Do not clobber if someone recreated the id.
    if sqlx::query_scalar::<_, Uuid>(r#"SELECT id FROM works WHERE id = $1"#)
        .bind(restored_id)
        .fetch_optional(pool)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "work {restored_id} already exists; cannot split"
        )));
    }

    let version_ids: Vec<Uuid> = snap.versions.iter().map(|v| v.id).collect();

    let mut tx = pool.begin().await?;

    // Recreate work row first with null primary (versions still on kept until reassigned).
    // primary_version_id FK is DEFERRABLE INITIALLY DEFERRED.
    sqlx::query(
        r#"
        INSERT INTO works (id, title_norm, primary_version_id, created_by, created_at)
        VALUES ($1, $2, NULL, $3, $4)
        "#,
    )
    .bind(restored_id)
    .bind(&snap.merged_work.title_norm)
    .bind(snap.merged_work.created_by)
    .bind(snap.merged_work.created_at)
    .execute(&mut *tx)
    .await?;

    if !version_ids.is_empty() {
        sqlx::query(
            r#"
            UPDATE versions SET work_id = $1
            WHERE id = ANY($2) AND work_id = $3
            "#,
        )
        .bind(restored_id)
        .bind(&version_ids)
        .bind(kept_work_id)
        .execute(&mut *tx)
        .await?;
    }

    // Restore primary_version_id on the recreated work.
    if let Some(pvid) = snap.merged_work.primary_version_id {
        sqlx::query(r#"UPDATE works SET primary_version_id = $1 WHERE id = $2"#)
            .bind(pvid)
            .bind(restored_id)
            .execute(&mut *tx)
            .await?;
    }

    // If kept's primary was one of the moved versions, repoint it.
    sqlx::query(
        r#"
        UPDATE works w
        SET primary_version_id = (
            SELECT v.id FROM versions v
            WHERE v.work_id = w.id
            ORDER BY v.created_at
            LIMIT 1
        )
        WHERE w.id = $1
          AND (
            w.primary_version_id IS NULL
            OR NOT EXISTS (
                SELECT 1 FROM versions v
                WHERE v.id = w.primary_version_id AND v.work_id = w.id
            )
          )
        "#,
    )
    .bind(kept_work_id)
    .execute(&mut *tx)
    .await?;

    if !snap.claim_ids.is_empty() {
        sqlx::query(r#"UPDATE claims SET work_id = $1 WHERE id = ANY($2) AND work_id = $3"#)
            .bind(restored_id)
            .bind(&snap.claim_ids)
            .bind(kept_work_id)
            .execute(&mut *tx)
            .await?;
    }
    if !snap.method_ids.is_empty() {
        sqlx::query(r#"UPDATE methods SET work_id = $1 WHERE id = ANY($2) AND work_id = $3"#)
            .bind(restored_id)
            .bind(&snap.method_ids)
            .bind(kept_work_id)
            .execute(&mut *tx)
            .await?;
    }
    if !snap.annotation_ids.is_empty() {
        sqlx::query(
            r#"UPDATE annotations SET work_id = $1 WHERE id = ANY($2) AND work_id = $3"#,
        )
        .bind(restored_id)
        .bind(&snap.annotation_ids)
        .bind(kept_work_id)
        .execute(&mut *tx)
        .await?;
    }
    if !snap.citation_ids_citing.is_empty() {
        sqlx::query(
            r#"
            UPDATE citations SET citing_work_id = $1
            WHERE id = ANY($2) AND citing_work_id = $3
            "#,
        )
        .bind(restored_id)
        .bind(&snap.citation_ids_citing)
        .bind(kept_work_id)
        .execute(&mut *tx)
        .await?;
    }
    if !snap.citation_ids_cited.is_empty() {
        sqlx::query(
            r#"
            UPDATE citations SET cited_work_id = $1
            WHERE id = ANY($2) AND cited_work_id = $3
            "#,
        )
        .bind(restored_id)
        .bind(&snap.citation_ids_cited)
        .bind(kept_work_id)
        .execute(&mut *tx)
        .await?;
    }

    // Reverse relation_member rewrites using snapshotted pre-merge keys.
    for m in &snap.relation_members {
        if m.entity_kind == "work" && m.entity_id == restored_id {
            // After merge entity_id became kept; try to move it back.
            let updated = sqlx::query(
                r#"
                UPDATE relation_members
                SET entity_id = $1
                WHERE relation_id = $2
                  AND entity_kind = 'work'
                  AND entity_id = $3
                  AND role = $4::member_role
                  AND NOT EXISTS (
                    SELECT 1 FROM relation_members x
                    WHERE x.relation_id = $2
                      AND x.entity_kind = 'work'
                      AND x.entity_id = $1
                      AND x.role = $4::member_role
                  )
                "#,
            )
            .bind(restored_id)
            .bind(m.relation_id)
            .bind(kept_work_id)
            .bind(&m.role)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if updated == 0 {
                // Member may have been deleted at merge due to PK conflict; re-insert.
                let anchor = m.anchor_work_id.map(|a| {
                    if a == restored_id {
                        restored_id
                    } else {
                        a
                    }
                });
                let _ = sqlx::query(
                    r#"
                    INSERT INTO relation_members
                        (relation_id, entity_kind, entity_id, role, anchor_work_id, position)
                    VALUES ($1, 'work', $2, $3::member_role, $4, $5)
                    ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(m.relation_id)
                .bind(restored_id)
                .bind(&m.role)
                .bind(anchor)
                .bind(m.position)
                .execute(&mut *tx)
                .await;
            }
        }
        if m.anchor_work_id == Some(restored_id) {
            // Prefer rows whose entity still matches the snapshot.
            let entity_id_for_update = if m.entity_kind == "work" && m.entity_id == restored_id {
                restored_id
            } else {
                m.entity_id
            };
            sqlx::query(
                r#"
                UPDATE relation_members
                SET anchor_work_id = $1
                WHERE relation_id = $2
                  AND entity_kind = $3::entity_kind
                  AND entity_id = $4
                  AND role = $5::member_role
                  AND anchor_work_id = $6
                "#,
            )
            .bind(restored_id)
            .bind(m.relation_id)
            .bind(&m.entity_kind)
            .bind(entity_id_for_update)
            .bind(&m.role)
            .bind(kept_work_id)
            .execute(&mut *tx)
            .await?;
            // Also try with entity_id still on kept for work-kind rows not yet moved.
            if m.entity_kind == "work" && m.entity_id == restored_id {
                sqlx::query(
                    r#"
                    UPDATE relation_members
                    SET anchor_work_id = $1
                    WHERE relation_id = $2
                      AND entity_kind = 'work'
                      AND entity_id = $3
                      AND role = $4::member_role
                      AND anchor_work_id = $5
                    "#,
                )
                .bind(restored_id)
                .bind(m.relation_id)
                .bind(kept_work_id)
                .bind(&m.role)
                .bind(kept_work_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    for pid in &snap.project_ids {
        sqlx::query(
            r#"
            INSERT INTO work_projects (work_id, project_id)
            VALUES ($1, $2) ON CONFLICT DO NOTHING
            "#,
        )
        .bind(restored_id)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }

    for rs in &snap.reading_status {
        sqlx::query(
            r#"
            INSERT INTO reading_status (user_id, work_id, status, starred, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id, work_id) DO NOTHING
            "#,
        )
        .bind(rs.user_id)
        .bind(restored_id)
        .bind(rs.status)
        .bind(rs.starred)
        .bind(rs.updated_at)
        .execute(&mut *tx)
        .await?;
    }

    // Restore neighbors that involved the merged work. Kept-side folds stay as-is.
    for n in &snap.neighbors {
        sqlx::query(
            r#"
            INSERT INTO neighbors (dimension, work_id, neighbor_work_id, score)
            VALUES ($1::neighbor_dimension, $2, $3, $4)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&n.dimension)
        .bind(n.work_id)
        .bind(n.neighbor_work_id)
        .bind(n.score)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"UPDATE merge_history SET reverted_at = now() WHERE id = $1 AND reverted_at IS NULL"#,
    )
    .bind(hist.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    get_work(pool, restored_id).await
}

async fn load_merge_history(
    pool: &PgPool,
    kept_work_id: Uuid,
    merge_history_id: Option<Uuid>,
    merged_work_id: Option<Uuid>,
) -> AppResult<MergeHistoryRow> {
    if let Some(hid) = merge_history_id {
        let row = sqlx::query_as::<_, MergeHistoryRow>(
            r#"
            SELECT id, kept_work_id, merged_work_id, snapshot, merged_by, created_at, reverted_at
            FROM merge_history WHERE id = $1
            "#,
        )
        .bind(hid)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("merge_history {hid}")))?;
        if row.kept_work_id != kept_work_id {
            return Err(AppError::BadRequest(
                "merge_history does not belong to this kept work".into(),
            ));
        }
        if let Some(mid) = merged_work_id {
            if row.merged_work_id != mid {
                return Err(AppError::BadRequest(
                    "merge_history.merged_work_id does not match request".into(),
                ));
            }
        }
        return Ok(row);
    }

    let mid = merged_work_id.expect("checked above");
    sqlx::query_as::<_, MergeHistoryRow>(
        r#"
        SELECT id, kept_work_id, merged_work_id, snapshot, merged_by, created_at, reverted_at
        FROM merge_history
        WHERE kept_work_id = $1 AND merged_work_id = $2 AND reverted_at IS NULL
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(kept_work_id)
    .bind(mid)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::NotFound(format!(
            "unreverted merge of {mid} into {kept_work_id}"
        ))
    })
}
