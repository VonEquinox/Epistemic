use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use epistemic_core::domain::{
    Annotation, AnnotationKind, ReadingLevel, ReadingStatusRow, Visibility,
};
use epistemic_core::repo::{annotations, reading};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::routes::relations::review_queue;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/review-queue", get(review_queue))
        .route("/works/{id}/reading-status", put(set_reading))
        .route(
            "/works/{id}/annotations",
            get(list_annotations).post(create_annotation),
        )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReadingReq {
    pub status: ReadingLevel,
    pub starred: Option<bool>,
}

async fn set_reading(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReadingReq>,
) -> ApiResult<Json<ReadingStatusRow>> {
    Ok(Json(
        reading::upsert(
            &state.pool,
            user.id,
            id,
            body.status,
            body.starred.unwrap_or(false),
        )
        .await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AnnotationReq {
    pub body: String,
    pub kind: Option<AnnotationKind>,
    pub visibility: Option<Visibility>,
    pub version_id: Option<Uuid>,
    pub anchor: Option<serde_json::Value>,
    pub parent_id: Option<Uuid>,
}

async fn create_annotation(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AnnotationReq>,
) -> ApiResult<Json<Annotation>> {
    Ok(Json(
        annotations::create(
            &state.pool,
            annotations::NewAnnotation {
                work_id: id,
                version_id: body.version_id,
                user_id: user.id,
                kind: body.kind.unwrap_or(AnnotationKind::Note),
                visibility: body.visibility.unwrap_or(Visibility::Team),
                anchor: body.anchor,
                body: body.body,
                parent_id: body.parent_id,
            },
        )
        .await?,
    ))
}

async fn list_annotations(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<Annotation>>> {
    Ok(Json(
        annotations::list_for_work(&state.pool, id, user.id).await?,
    ))
}
