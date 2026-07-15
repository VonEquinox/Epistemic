use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    Claim, ClaimJudgment, ClaimVerdict, EvidenceSpan, Review, ReviewStatus, ReviewVerdict,
    SourceLayer, SubjectKind,
};
use crate::error::{AppError, AppResult};
use crate::repo::{evidence, relations};

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ClaimJudgmentDetail {
    pub judgment: ClaimJudgment,
    pub reviews: Vec<Review>,
    /// True when both agree and disagree reviews exist on this judgment.
    pub disputed: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ClaimWithEvidence {
    pub claim: Claim,
    pub evidence: Vec<EvidenceSpan>,
    /// Flat judgments (backward-compatible). Use GET /claims/{id}/judgments for reviews.
    pub judgments: Vec<ClaimJudgment>,
}

#[derive(Debug, Clone)]
pub struct NewClaim {
    pub work_id: Uuid,
    pub text: String,
    pub source: SourceLayer,
    pub review_status: ReviewStatus,
    pub created_by: Option<Uuid>,
    pub model_version: Option<String>,
    pub evidence: Vec<evidence::NewEvidenceSpan>,
}

pub async fn create(pool: &PgPool, nc: NewClaim) -> AppResult<ClaimWithEvidence> {
    if nc.text.trim().is_empty() {
        return Err(AppError::Validation("claim text required".into()));
    }
    let claim = sqlx::query_as::<_, Claim>(
        r#"
        INSERT INTO claims (work_id, text, source, review_status, created_by, model_version)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, work_id, text, source, review_status, created_by, model_version, created_at
        "#,
    )
    .bind(nc.work_id)
    .bind(nc.text)
    .bind(nc.source)
    .bind(nc.review_status)
    .bind(nc.created_by)
    .bind(nc.model_version)
    .fetch_one(pool)
    .await?;

    let mut evs = Vec::new();
    for mut e in nc.evidence {
        e.claim_id = Some(claim.id);
        // discard empty evidence (principle 2)
        if e.text.trim().is_empty() {
            continue;
        }
        evs.push(evidence::create(pool, e).await?);
    }

    Ok(ClaimWithEvidence {
        claim,
        evidence: evs,
        judgments: vec![],
    })
}

pub async fn get_with_evidence(pool: &PgPool, id: Uuid) -> AppResult<ClaimWithEvidence> {
    let claim = sqlx::query_as::<_, Claim>(
        r#"
        SELECT id, work_id, text, source, review_status, created_by, model_version, created_at
        FROM claims WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("claim {id}")))?;

    let evs = evidence::list_for_claim(pool, id).await?;
    let judgments = list_judgments(pool, id).await?;
    Ok(ClaimWithEvidence {
        claim,
        evidence: evs,
        judgments,
    })
}

pub async fn list_with_evidence_for_work(
    pool: &PgPool,
    work_id: Uuid,
) -> AppResult<Vec<ClaimWithEvidence>> {
    let claims = sqlx::query_as::<_, Claim>(
        r#"
        SELECT id, work_id, text, source, review_status, created_by, model_version, created_at
        FROM claims WHERE work_id = $1 ORDER BY created_at
        "#,
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(claims.len());
    for claim in claims {
        let evidence = evidence::list_for_claim(pool, claim.id).await?;
        let judgments = list_judgments(pool, claim.id).await?;
        out.push(ClaimWithEvidence {
            claim,
            evidence,
            judgments,
        });
    }
    Ok(out)
}

pub async fn add_judgment(
    pool: &PgPool,
    claim_id: Uuid,
    user_id: Uuid,
    verdict: ClaimVerdict,
    conditions: &str,
    evidence_url: Option<String>,
) -> AppResult<ClaimJudgment> {
    // ensure claim exists
    let _: Uuid = sqlx::query_scalar(r#"SELECT id FROM claims WHERE id = $1"#)
        .bind(claim_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("claim {claim_id}")))?;

    let row = sqlx::query_as::<_, ClaimJudgment>(
        r#"
        INSERT INTO claim_judgments (claim_id, user_id, verdict, conditions, evidence_url)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, claim_id, user_id, verdict, conditions, evidence_url, created_at
        "#,
    )
    .bind(claim_id)
    .bind(user_id)
    .bind(verdict)
    .bind(conditions)
    .bind(evidence_url)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_judgment(pool: &PgPool, judgment_id: Uuid) -> AppResult<ClaimJudgment> {
    sqlx::query_as::<_, ClaimJudgment>(
        r#"
        SELECT id, claim_id, user_id, verdict, conditions, evidence_url, created_at
        FROM claim_judgments WHERE id = $1
        "#,
    )
    .bind(judgment_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("claim_judgment {judgment_id}")))
}

pub async fn get_judgment_detail(
    pool: &PgPool,
    judgment_id: Uuid,
) -> AppResult<ClaimJudgmentDetail> {
    let judgment = get_judgment(pool, judgment_id).await?;
    let reviews =
        relations::list_reviews(pool, SubjectKind::ClaimJudgment, judgment_id).await?;
    let disputed = is_disputed(&reviews);
    Ok(ClaimJudgmentDetail {
        judgment,
        reviews,
        disputed,
    })
}

pub async fn list_judgments(pool: &PgPool, claim_id: Uuid) -> AppResult<Vec<ClaimJudgment>> {
    // ensure claim exists
    let _: Uuid = sqlx::query_scalar(r#"SELECT id FROM claims WHERE id = $1"#)
        .bind(claim_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("claim {claim_id}")))?;

    let rows = sqlx::query_as::<_, ClaimJudgment>(
        r#"
        SELECT id, claim_id, user_id, verdict, conditions, evidence_url, created_at
        FROM claim_judgments WHERE claim_id = $1 ORDER BY created_at DESC
        "#,
    )
    .bind(claim_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_judgments_detailed(
    pool: &PgPool,
    claim_id: Uuid,
) -> AppResult<Vec<ClaimJudgmentDetail>> {
    let judgments = list_judgments(pool, claim_id).await?;
    let mut out = Vec::with_capacity(judgments.len());
    for judgment in judgments {
        let reviews =
            relations::list_reviews(pool, SubjectKind::ClaimJudgment, judgment.id).await?;
        let disputed = is_disputed(&reviews);
        out.push(ClaimJudgmentDetail {
            judgment,
            reviews,
            disputed,
        });
    }
    Ok(out)
}

/// Review a claim_judgment (agree/disagree). Opposite reviews → claim.review_status = disputed.
pub async fn add_judgment_review(
    pool: &PgPool,
    judgment_id: Uuid,
    user_id: Uuid,
    verdict: ReviewVerdict,
    comment: &str,
) -> AppResult<ClaimJudgmentDetail> {
    let judgment = get_judgment(pool, judgment_id).await?;

    relations::upsert_review(
        pool,
        SubjectKind::ClaimJudgment,
        judgment_id,
        user_id,
        verdict,
        comment,
    )
    .await?;

    recompute_claim_status_from_judgments(pool, judgment.claim_id).await?;

    get_judgment_detail(pool, judgment_id).await
}

fn is_disputed(reviews: &[Review]) -> bool {
    let agrees = reviews
        .iter()
        .any(|r| r.verdict == ReviewVerdict::Agree);
    let disagrees = reviews
        .iter()
        .any(|r| r.verdict == ReviewVerdict::Disagree);
    agrees && disagrees
}

/// When any judgment on the claim has opposite reviews, mark claim disputed.
/// If no judgment is disputed, leave claim status alone (do not auto-confirm/reject).
async fn recompute_claim_status_from_judgments(
    pool: &PgPool,
    claim_id: Uuid,
) -> AppResult<()> {
    let judgments = list_judgments(pool, claim_id).await?;
    let mut any_disputed = false;
    for j in &judgments {
        let reviews =
            relations::list_reviews(pool, SubjectKind::ClaimJudgment, j.id).await?;
        if is_disputed(&reviews) {
            any_disputed = true;
            break;
        }
    }

    if any_disputed {
        sqlx::query(r#"UPDATE claims SET review_status = $2 WHERE id = $1"#)
            .bind(claim_id)
            .bind(ReviewStatus::Disputed)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Promote annotation selection text into a claim (+ evidence span).
pub async fn promote_from_selection(
    pool: &PgPool,
    work_id: Uuid,
    version_id: Uuid,
    user_id: Uuid,
    claim_text: &str,
    source_text: &str,
    page: i32,
    bbox: Option<serde_json::Value>,
) -> AppResult<ClaimWithEvidence> {
    create(
        pool,
        NewClaim {
            work_id,
            text: claim_text.to_string(),
            source: SourceLayer::TeamRecord,
            review_status: ReviewStatus::Confirmed,
            created_by: Some(user_id),
            model_version: None,
            evidence: vec![evidence::NewEvidenceSpan {
                relation_id: None,
                claim_id: None,
                extraction_field: Some("promoted_claim".into()),
                version_id,
                page,
                text: source_text.to_string(),
                bbox,
            }],
        },
    )
    .await
}
