use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::jobs;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
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
    AuthUser(_): AuthUser,
    Json(body): Json<BatchDnaReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if body.version_ids.is_empty() {
        return Ok(Json(serde_json::json!({ "ok": false, "error": "empty" })));
    }
    let job = jobs::enqueue(
        &state.pool,
        job_kind::BATCH_ORCH,
        serde_json::json!({
            "version_ids": body.version_ids.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            "kind": "extract_dna"
        }),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true, "job_id": job.id })))
}

async fn list_for_work(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
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
    AuthUser(_): AuthUser,
    Json(body): Json<RequeueReq>,
) -> ApiResult<Json<Job>> {
    Ok(Json(
        jobs::requeue(
            &state.pool,
            &body.kind,
            body.version_id,
            body.work_id,
        )
        .await?,
    ))
}
