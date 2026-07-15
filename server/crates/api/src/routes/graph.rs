use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use epistemic_core::repo::graph;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/map", get(map))
        .route("/ego/{kind}/{id}", get(ego))
}

async fn map(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<graph::MapResponse>> {
    Ok(Json(graph::map_data(&state.pool).await?))
}

#[derive(Debug, Deserialize)]
pub struct EgoQuery {
    pub depth: Option<i32>,
    #[allow(dead_code)]
    pub mode: Option<String>,
}

async fn ego(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path((kind, id)): Path<(String, Uuid)>,
    Query(q): Query<EgoQuery>,
) -> ApiResult<Json<graph::EgoResponse>> {
    let depth = q.depth.unwrap_or(1).clamp(1, 2);
    // MVP: only work ego fully implemented
    match kind.as_str() {
        "work" => Ok(Json(graph::ego_work(&state.pool, id, depth).await?)),
        _ => {
            // Fallback: treat as work id for other kinds for now
            Ok(Json(graph::ego_work(&state.pool, id, depth).await?))
        }
    }
}
