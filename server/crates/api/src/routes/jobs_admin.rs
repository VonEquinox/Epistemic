use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use epistemic_core::domain::job_kind;
use epistemic_core::repo::jobs;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/batch-dna", post(batch_dna))
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
