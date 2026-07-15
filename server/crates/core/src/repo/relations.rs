use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    job_kind, EntityKind, EvidenceSpan, MemberRole, Relation, RelationDetail, RelationMember,
    RelationType, Review, ReviewStatus, ReviewVerdict, SourceLayer, SubjectKind,
};
use crate::error::{AppError, AppResult};
use crate::repo::jobs;

#[derive(Debug, Clone)]
pub struct NewRelationMember {
    pub entity_kind: EntityKind,
    pub entity_id: Uuid,
    pub role: MemberRole,
    pub anchor_work_id: Option<Uuid>,
    pub position: i32,
}

#[derive(Debug, Clone)]
pub struct NewEvidence {
    pub version_id: Uuid,
    pub page: i32,
    pub text: String,
    pub bbox: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct NewRelation {
    pub relation_type: RelationType,
    pub aspect: Option<String>,
    pub scope: Option<String>,
    pub explanation: String,
    pub confidence: Option<f64>,
    pub source: SourceLayer,
    pub review_status: ReviewStatus,
    pub created_by_user: Option<Uuid>,
    pub model_version: Option<String>,
    pub members: Vec<NewRelationMember>,
    pub evidence: Vec<NewEvidence>,
}

pub async fn create_relation(pool: &PgPool, nr: NewRelation) -> AppResult<RelationDetail> {
    let mut tx = pool.begin().await?;

    let rel = sqlx::query_as::<_, Relation>(
        r#"
        INSERT INTO relations (
            type, aspect, scope, explanation, confidence,
            source, review_status, created_by_user, model_version
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, type, aspect, scope, explanation, confidence,
                  source, review_status, created_by_user, model_version, created_at
        "#,
    )
    .bind(nr.relation_type)
    .bind(&nr.aspect)
    .bind(&nr.scope)
    .bind(&nr.explanation)
    .bind(nr.confidence)
    .bind(nr.source)
    .bind(nr.review_status)
    .bind(nr.created_by_user)
    .bind(&nr.model_version)
    .fetch_one(&mut *tx)
    .await?;

    let mut members = Vec::with_capacity(nr.members.len());
    for m in &nr.members {
        let row = sqlx::query_as::<_, RelationMember>(
            r#"
            INSERT INTO relation_members (
                relation_id, entity_kind, entity_id, role, anchor_work_id, position
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING relation_id, entity_kind, entity_id, role, anchor_work_id, position
            "#,
        )
        .bind(rel.id)
        .bind(m.entity_kind)
        .bind(m.entity_id)
        .bind(m.role)
        .bind(m.anchor_work_id)
        .bind(m.position)
        .fetch_one(&mut *tx)
        .await?;
        members.push(row);
    }

    let mut evidence = Vec::with_capacity(nr.evidence.len());
    for e in &nr.evidence {
        let row = sqlx::query_as::<_, EvidenceSpan>(
            r#"
            INSERT INTO evidence_spans (relation_id, version_id, page, text, bbox)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, relation_id, claim_id, extraction_field, version_id,
                      page, text, bbox, created_at
            "#,
        )
        .bind(rel.id)
        .bind(e.version_id)
        .bind(e.page)
        .bind(&e.text)
        .bind(&e.bbox)
        .fetch_one(&mut *tx)
        .await?;
        evidence.push(row);
    }

    tx.commit().await?;

    if nr.relation_type.is_method_lineage() {
        let _ = jobs::enqueue(
            pool,
            job_kind::UPDATE_NEIGHBORS_LINEAGE,
            serde_json::json!({ "relation_id": rel.id }),
        )
        .await;
    }

    Ok(RelationDetail {
        relation: rel,
        members,
        evidence,
        reviews: vec![],
    })
}

pub async fn get_relation(pool: &PgPool, id: Uuid) -> AppResult<RelationDetail> {
    let rel = sqlx::query_as::<_, Relation>(
        r#"
        SELECT id, type, aspect, scope, explanation, confidence,
               source, review_status, created_by_user, model_version, created_at
        FROM relations WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("relation {id}")))?;

    let members = sqlx::query_as::<_, RelationMember>(
        r#"
        SELECT relation_id, entity_kind, entity_id, role, anchor_work_id, position
        FROM relation_members WHERE relation_id = $1 ORDER BY position
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let evidence = sqlx::query_as::<_, EvidenceSpan>(
        r#"
        SELECT id, relation_id, claim_id, extraction_field, version_id,
               page, text, bbox, created_at
        FROM evidence_spans WHERE relation_id = $1
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let reviews = sqlx::query_as::<_, Review>(
        r#"
        SELECT id, subject_kind, subject_id, user_id, verdict, comment, created_at
        FROM reviews WHERE subject_kind = 'relation' AND subject_id = $1
        ORDER BY created_at
        "#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(RelationDetail {
        relation: rel,
        members,
        evidence,
        reviews,
    })
}

pub async fn patch_relation(
    pool: &PgPool,
    id: Uuid,
    relation_type: Option<RelationType>,
    aspect: Option<String>,
    explanation: Option<String>,
    swap_direction: bool,
) -> AppResult<RelationDetail> {
    if let Some(rt) = relation_type {
        sqlx::query(r#"UPDATE relations SET type = $2 WHERE id = $1"#)
            .bind(id)
            .bind(rt)
            .execute(pool)
            .await?;
    }
    if aspect.is_some() {
        sqlx::query(r#"UPDATE relations SET aspect = $2 WHERE id = $1"#)
            .bind(id)
            .bind(aspect)
            .execute(pool)
            .await?;
    }
    if let Some(exp) = explanation {
        sqlx::query(r#"UPDATE relations SET explanation = $2 WHERE id = $1"#)
            .bind(id)
            .bind(exp)
            .execute(pool)
            .await?;
    }
    if swap_direction {
        sqlx::query(
            r#"
            UPDATE relation_members
            SET role = CASE role
                WHEN 'source' THEN 'target'::member_role
                WHEN 'target' THEN 'source'::member_role
                ELSE role
            END
            WHERE relation_id = $1
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;
    }
    get_relation(pool, id).await
}

pub async fn add_review(
    pool: &PgPool,
    relation_id: Uuid,
    user_id: Uuid,
    verdict: ReviewVerdict,
    comment: &str,
) -> AppResult<RelationDetail> {
    let _ = get_relation(pool, relation_id).await?;

    sqlx::query(
        r#"
        INSERT INTO reviews (subject_kind, subject_id, user_id, verdict, comment)
        VALUES ('relation', $1, $2, $3, $4)
        ON CONFLICT (subject_kind, subject_id, user_id) DO UPDATE
        SET verdict = EXCLUDED.verdict, comment = EXCLUDED.comment, created_at = now()
        "#,
    )
    .bind(relation_id)
    .bind(user_id)
    .bind(verdict)
    .bind(comment)
    .execute(pool)
    .await?;

    recompute_status(pool, relation_id).await?;

    let _ = jobs::enqueue(
        pool,
        job_kind::UPDATE_NEIGHBORS_LINEAGE,
        serde_json::json!({ "relation_id": relation_id }),
    )
    .await;

    get_relation(pool, relation_id).await
}

async fn recompute_status(pool: &PgPool, relation_id: Uuid) -> AppResult<()> {
    #[derive(sqlx::FromRow)]
    struct V {
        verdict: ReviewVerdict,
    }
    let rows = sqlx::query_as::<_, V>(
        r#"
        SELECT verdict FROM reviews
        WHERE subject_kind = 'relation' AND subject_id = $1
        "#,
    )
    .bind(relation_id)
    .fetch_all(pool)
    .await?;

    let agrees = rows
        .iter()
        .filter(|r| r.verdict == ReviewVerdict::Agree)
        .count();
    let disagrees = rows
        .iter()
        .filter(|r| r.verdict == ReviewVerdict::Disagree)
        .count();

    let status = if disagrees > 0 && agrees > 0 {
        ReviewStatus::Disputed
    } else if disagrees > 0 && agrees == 0 {
        ReviewStatus::Rejected
    } else if agrees > 0 {
        ReviewStatus::Confirmed
    } else {
        ReviewStatus::Unreviewed
    };

    sqlx::query(r#"UPDATE relations SET review_status = $2 WHERE id = $1"#)
        .bind(relation_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_review_status(
    pool: &PgPool,
    relation_id: Uuid,
    status: ReviewStatus,
    user_id: Uuid,
) -> AppResult<RelationDetail> {
    let verdict = match status {
        ReviewStatus::Confirmed => ReviewVerdict::Agree,
        ReviewStatus::Rejected => ReviewVerdict::Disagree,
        other => {
            sqlx::query(r#"UPDATE relations SET review_status = $2 WHERE id = $1"#)
                .bind(relation_id)
                .bind(other)
                .execute(pool)
                .await?;
            return get_relation(pool, relation_id).await;
        }
    };
    add_review(pool, relation_id, user_id, verdict, "").await
}

#[derive(Debug, Clone, Default)]
pub struct ReviewQueueQuery {
    pub work_id: Option<Uuid>,
    pub only_unreviewed: bool,
    pub limit: i64,
    pub offset: i64,
}

pub async fn review_queue(pool: &PgPool, q: ReviewQueueQuery) -> AppResult<Vec<RelationDetail>> {
    let limit = if q.limit <= 0 { 50 } else { q.limit.min(200) };
    let offset = q.offset.max(0);

    let ids: Vec<Uuid> = match (q.work_id, q.only_unreviewed) {
        (Some(wid), true) => {
            sqlx::query_scalar(
                r#"
                SELECT DISTINCT r.id FROM relations r
                JOIN relation_members rm ON rm.relation_id = r.id
                WHERE rm.anchor_work_id = $1
                  AND r.review_status = 'unreviewed'
                  AND r.source = 'ai_candidate'
                ORDER BY r.id LIMIT $2 OFFSET $3
                "#,
            )
            .bind(wid)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        (Some(wid), false) => {
            sqlx::query_scalar(
                r#"
                SELECT DISTINCT r.id FROM relations r
                JOIN relation_members rm ON rm.relation_id = r.id
                WHERE rm.anchor_work_id = $1 AND r.source = 'ai_candidate'
                ORDER BY r.id LIMIT $2 OFFSET $3
                "#,
            )
            .bind(wid)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        (None, true) => {
            // Default queue: conf ≥ 0.75, plus all high-risk types regardless of conf
            // (PRD §6.4). Mid-band (0.5–0.75) available via ?all=true.
            sqlx::query_scalar(
                r#"
                SELECT r.id FROM relations r
                WHERE r.review_status = 'unreviewed'
                  AND r.source = 'ai_candidate'
                  AND r.type NOT IN ('cites', 'version_of')
                  AND (
                    COALESCE(r.confidence, 0) >= 0.75
                    OR r.type IN ('fails_to_reproduce', 'contradicts_claim')
                  )
                ORDER BY r.confidence DESC NULLS LAST, r.created_at
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        (None, false) => {
            sqlx::query_scalar(
                r#"
                SELECT r.id FROM relations r
                WHERE r.source = 'ai_candidate' AND r.type NOT IN ('cites')
                ORDER BY r.created_at DESC LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
    };

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(get_relation(pool, id).await?);
    }
    Ok(out)
}

// silence
#[allow(dead_code)]
fn _use(_: SubjectKind) {}
