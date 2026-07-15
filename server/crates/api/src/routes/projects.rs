use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::Project;
use epistemic_core::repo::projects;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one))
        .route("/{id}/coverage", get(coverage))
        .route("/{id}/works/{work_id}", post(attach))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Vec<Project>>> {
    Ok(Json(projects::list_projects(&state.pool).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProjectReq {
    pub name: String,
    pub description: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Json(body): Json<CreateProjectReq>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        projects::create_project(
            &state.pool,
            &body.name,
            body.description.as_deref().unwrap_or(""),
        )
        .await?,
    ))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Project>> {
    Ok(Json(projects::get_project(&state.pool, id).await?))
}

async fn coverage(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<projects::CoverageEntry>>> {
    Ok(Json(projects::project_coverage(&state.pool, id).await?))
}

async fn attach(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path((id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    projects::attach_work(&state.pool, work_id, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
