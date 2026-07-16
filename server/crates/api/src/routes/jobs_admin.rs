use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::jobs;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AdminUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/batch-dna", post(batch_dna))
        .route("/work/{work_id}", get(list_for_work))
        .route("/requeue", post(requeue_job))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchDnaReq {
    pub version_ids: Vec<Uuid>,
}

async fn batch_dna(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(body): Json<BatchDnaReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if body.version_ids.is_empty() {
        return Err(epistemic_core::AppError::BadRequest("version_ids is empty".into()).into());
    }
    if body.version_ids.len() > 500 {
        return Err(
            epistemic_core::AppError::BadRequest("batch size exceeds 500 versions".into()).into(),
        );
    }
    let mut version_ids = body.version_ids;
    version_ids.sort_unstable();
    version_ids.dedup();
    let mut hasher = Sha256::new();
    for version_id in &version_ids {
        hasher.update(version_id.as_bytes());
    }
    let dedupe_key = format!("batch-dna:{:x}", hasher.finalize());
    let job = jobs::enqueue_unique(
        &state.pool,
        job_kind::BATCH_ORCH,
        serde_json::json!({
            "version_ids": version_ids.iter().map(Uuid::to_string).collect::<Vec<_>>(),
            "kind": "extract_dna"
        }),
        &dedupe_key,
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true, "job_id": job.id })))
}

async fn list_for_work(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(work_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Job>>> {
    Ok(Json(jobs::jobs_for_work(&state.pool, work_id).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RequeueReq {
    pub kind: String,
    pub version_id: Option<Uuid>,
    pub work_id: Option<Uuid>,
}

async fn requeue_job(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(body): Json<RequeueReq>,
) -> ApiResult<Json<Job>> {
    const ALLOWED_KINDS: &[&str] = &[
        job_kind::RESOLVE_METADATA,
        job_kind::FETCH_PDF,
        job_kind::EXTRACT_DNA,
        job_kind::FETCH_REFERENCES,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        job_kind::UPDATE_NEIGHBORS_LINEAGE,
        job_kind::CLASSIFY_CITATION_CONTEXTS,
        job_kind::EMBED,
        job_kind::PROPOSE_PAIRS,
    ];
    if !ALLOWED_KINDS.contains(&body.kind.as_str()) {
        return Err(epistemic_core::AppError::BadRequest("unsupported job kind".into()).into());
    }
    Ok(Json(
        jobs::requeue(&state.pool, &body.kind, body.version_id, body.work_id).await?,
    ))
}
