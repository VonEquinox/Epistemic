use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{ClaimJudgment, ClaimVerdict, ReviewStatus, SourceLayer};
use epistemic_core::repo::{claims, evidence};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_claim))
        .route("/{id}", get(get_claim))
        .route("/{id}/judgments", post(add_judgment))
        .route("/promote", post(promote))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateClaimReq {
    pub work_id: Uuid,
    pub text: String,
    pub version_id: Option<Uuid>,
    pub page: Option<i32>,
    pub source_text: Option<String>,
    pub bbox: Option<serde_json::Value>,
}

async fn create_claim(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateClaimReq>,
) -> ApiResult<Json<claims::ClaimWithEvidence>> {
    let mut evs = vec![];
    if let (Some(vid), Some(page), Some(src)) =
        (body.version_id, body.page, body.source_text.clone())
    {
        evs.push(evidence::NewEvidenceSpan {
            relation_id: None,
            claim_id: None,
            extraction_field: Some("manual".into()),
            version_id: vid,
            page,
            text: src,
            bbox: body.bbox,
        });
    }
    Ok(Json(
        claims::create(
            &state.pool,
            claims::NewClaim {
                work_id: body.work_id,
                text: body.text,
                source: SourceLayer::TeamRecord,
                review_status: ReviewStatus::Confirmed,
                created_by: Some(user.id),
                model_version: None,
                evidence: evs,
            },
        )
        .await?,
    ))
}

async fn get_claim(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<claims::ClaimWithEvidence>> {
    Ok(Json(claims::get_with_evidence(&state.pool, id).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct JudgmentReq {
    pub verdict: ClaimVerdict,
    pub conditions: Option<String>,
    pub evidence_url: Option<String>,
}

async fn add_judgment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<JudgmentReq>,
) -> ApiResult<Json<ClaimJudgment>> {
    Ok(Json(
        claims::add_judgment(
            &state.pool,
            id,
            user.id,
            body.verdict,
            body.conditions.as_deref().unwrap_or(""),
            body.evidence_url,
        )
        .await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PromoteReq {
    pub work_id: Uuid,
    pub version_id: Uuid,
    pub claim_text: String,
    pub source_text: String,
    pub page: i32,
    pub bbox: Option<serde_json::Value>,
}

async fn promote(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<PromoteReq>,
) -> ApiResult<Json<claims::ClaimWithEvidence>> {
    Ok(Json(
        claims::promote_from_selection(
            &state.pool,
            body.work_id,
            body.version_id,
            user.id,
            &body.claim_text,
            &body.source_text,
            body.page,
            body.bbox,
        )
        .await?,
    ))
}

pub async fn list_for_work(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(work_id): Path<Uuid>,
) -> ApiResult<Json<Vec<claims::ClaimWithEvidence>>> {
    Ok(Json(
        claims::list_with_evidence_for_work(&state.pool, work_id).await?,
    ))
}
