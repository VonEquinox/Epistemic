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
    /// explore | prerequisite | dispute | evolution
    pub mode: Option<String>,
}

async fn ego(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path((kind, id)): Path<(String, Uuid)>,
    Query(q): Query<EgoQuery>,
) -> ApiResult<Json<graph::EgoResponse>> {
    let depth = q.depth.unwrap_or(1).clamp(1, 2);
    let mode = q.mode.as_deref().unwrap_or("explore");
    match kind.as_str() {
        "work" | "claim" | "method" | "dataset" => {
            // MVP: claim/method/dataset resolve via their work when possible;
            // for now treat id as work_id (claim ego refinement is post-M3).
            Ok(Json(
                graph::ego_work_mode(&state.pool, id, depth, mode).await?,
            ))
        }
        _ => Ok(Json(
            graph::ego_work_mode(&state.pool, id, depth, mode).await?,
        )),
    }
}
