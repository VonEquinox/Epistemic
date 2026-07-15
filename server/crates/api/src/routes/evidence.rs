use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::EvidenceSpan;
use epistemic_core::repo::{claims, evidence};
use epistemic_core::AppError;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_evidence))
        .route("/{id}", get(get_one))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEvidenceReq {
    pub version_id: Uuid,
    pub page: i32,
    pub text: String,
    pub bbox: Option<serde_json::Value>,
    pub relation_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub extraction_field: Option<String>,
}

async fn create_evidence(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Json(body): Json<CreateEvidenceReq>,
) -> ApiResult<Json<EvidenceSpan>> {
    // verify version exists
    let _ = epistemic_core::repo::works::get_version(&state.pool, body.version_id).await?;
    Ok(Json(
        evidence::create(
            &state.pool,
            evidence::NewEvidenceSpan {
                relation_id: body.relation_id,
                claim_id: body.claim_id,
                extraction_field: body.extraction_field,
                version_id: body.version_id,
                page: body.page,
                text: body.text,
                bbox: body.bbox,
            },
        )
        .await?,
    ))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<EvidenceSpan>> {
    Ok(Json(evidence::get(&state.pool, id).await?))
}

#[derive(Debug, Deserialize)]
pub struct WorkEvidenceQuery {
    pub version_id: Option<Uuid>,
}

pub async fn list_for_work(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(work_id): Path<Uuid>,
    Query(q): Query<WorkEvidenceQuery>,
) -> ApiResult<Json<Vec<EvidenceSpan>>> {
    let rows = if let Some(vid) = q.version_id {
        evidence::list_for_version(&state.pool, vid).await?
    } else {
        evidence::list_for_work(&state.pool, work_id).await?
    };
    Ok(Json(rows))
}

// silence
#[allow(dead_code)]
fn _use_claims() {
    let _ = claims::list_with_evidence_for_work;
    let _ = AppError::NotFound("".into());
}
