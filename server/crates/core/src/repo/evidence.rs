use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::EvidenceSpan;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct NewEvidenceSpan {
    pub relation_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub extraction_field: Option<String>,
    pub version_id: Uuid,
    pub page: i32,
    pub text: String,
    pub bbox: Option<serde_json::Value>,
}

pub async fn create(pool: &PgPool, ne: NewEvidenceSpan) -> AppResult<EvidenceSpan> {
    if ne.relation_id.is_none() && ne.claim_id.is_none() && ne.extraction_field.is_none() {
        return Err(AppError::Validation(
            "evidence must bind to relation, claim, or extraction_field".into(),
        ));
    }
    if ne.page < 1 {
        return Err(AppError::Validation("page must be ≥ 1".into()));
    }
    if ne.text.trim().is_empty() {
        return Err(AppError::Validation("evidence text required".into()));
    }

    let row = sqlx::query_as::<_, EvidenceSpan>(
        r#"
        INSERT INTO evidence_spans (
            relation_id, claim_id, extraction_field, version_id, page, text, bbox
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, relation_id, claim_id, extraction_field, version_id,
                  page, text, bbox, created_at
        "#,
    )
    .bind(ne.relation_id)
    .bind(ne.claim_id)
    .bind(ne.extraction_field)
    .bind(ne.version_id)
    .bind(ne.page)
    .bind(ne.text)
    .bind(ne.bbox)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<EvidenceSpan> {
    sqlx::query_as::<_, EvidenceSpan>(
        r#"
        SELECT id, relation_id, claim_id, extraction_field, version_id,
               page, text, bbox, created_at
        FROM evidence_spans WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("evidence {id}")))
}

pub async fn list_for_claim(pool: &PgPool, claim_id: Uuid) -> AppResult<Vec<EvidenceSpan>> {
    let rows = sqlx::query_as::<_, EvidenceSpan>(
        r#"
        SELECT id, relation_id, claim_id, extraction_field, version_id,
               page, text, bbox, created_at
        FROM evidence_spans WHERE claim_id = $1
        ORDER BY page, created_at
        "#,
    )
    .bind(claim_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_for_version(pool: &PgPool, version_id: Uuid) -> AppResult<Vec<EvidenceSpan>> {
    let rows = sqlx::query_as::<_, EvidenceSpan>(
        r#"
        SELECT id, relation_id, claim_id, extraction_field, version_id,
               page, text, bbox, created_at
        FROM evidence_spans WHERE version_id = $1
        ORDER BY page, created_at
        "#,
    )
    .bind(version_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_for_work(pool: &PgPool, work_id: Uuid) -> AppResult<Vec<EvidenceSpan>> {
    // Evidence on claims of this work + relations anchored to this work + field extractions on its versions
    let rows = sqlx::query_as::<_, EvidenceSpan>(
        r#"
        SELECT DISTINCT e.id, e.relation_id, e.claim_id, e.extraction_field, e.version_id,
               e.page, e.text, e.bbox, e.created_at
        FROM evidence_spans e
        JOIN versions v ON v.id = e.version_id
        WHERE v.work_id = $1
           OR e.claim_id IN (SELECT id FROM claims WHERE work_id = $1)
           OR e.relation_id IN (
                SELECT rm.relation_id FROM relation_members rm
                WHERE rm.anchor_work_id = $1
           )
        ORDER BY e.page, e.created_at
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_for_relation(pool: &PgPool, relation_id: Uuid) -> AppResult<Vec<EvidenceSpan>> {
    let rows = sqlx::query_as::<_, EvidenceSpan>(
        r#"
        SELECT id, relation_id, claim_id, extraction_field, version_id,
               page, text, bbox, created_at
        FROM evidence_spans WHERE relation_id = $1
        ORDER BY page, created_at
        "#,
    )
    .bind(relation_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
