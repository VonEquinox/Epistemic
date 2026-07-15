use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use epistemic_core::repo::views::{self, SavedView};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one).delete(delete_one))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Vec<SavedView>>> {
    Ok(Json(views::list(&state.pool).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateViewReq {
    pub name: String,
    pub weights: serde_json::Value,
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateViewReq>,
) -> ApiResult<Json<SavedView>> {
    Ok(Json(
        views::create(&state.pool, &body.name, body.weights, user.id).await?,
    ))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<SavedView>> {
    Ok(Json(views::get(&state.pool, id).await?))
}

async fn delete_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    views::delete(&state.pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
