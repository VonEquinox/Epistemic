use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use epistemic_core::domain::{
    Annotation, AnnotationKind, CommentKind, NodeComment, ReadingLevel, ReadingStatusRow,
    Visibility,
};
use epistemic_core::repo::{annotations, comments, groups, reading};
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
        .route(
            "/annotations/{id}",
            axum::routing::delete(delete_annotation),
        )
        .route(
            "/graphs/{graph_id}/works/{work_id}/comments",
            get(list_node_comments).post(create_node_comment),
        )
        .route(
            "/comments/{id}",
            axum::routing::patch(update_node_comment).delete(delete_node_comment),
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

async fn delete_annotation(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    // Ensure it exists (clearer error) then author-only delete.
    let _ = annotations::get(&state.pool, id).await?;
    annotations::delete(&state.pool, id, user.id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NodeCommentReq {
    pub body: String,
    pub kind: Option<CommentKind>,
    pub visibility: Option<Visibility>,
    pub parent_id: Option<Uuid>,
}

async fn list_node_comments(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((graph_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Vec<NodeComment>>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    Ok(Json(
        comments::list_for_node(&state.pool, graph_id, work_id, user.id).await?,
    ))
}

async fn create_node_comment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((graph_id, work_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<NodeCommentReq>,
) -> ApiResult<Json<NodeComment>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    Ok(Json(
        comments::create(
            &state.pool,
            comments::NewNodeComment {
                graph_id,
                work_id,
                user_id: user.id,
                kind: body.kind.unwrap_or(CommentKind::Comment),
                visibility: body.visibility.unwrap_or(Visibility::Team),
                body: body.body,
                parent_id: body.parent_id,
            },
        )
        .await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NodeCommentPatchReq {
    pub body: Option<String>,
    pub kind: Option<CommentKind>,
    pub visibility: Option<Visibility>,
}

async fn update_node_comment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<NodeCommentPatchReq>,
) -> ApiResult<Json<NodeComment>> {
    let existing = comments::get(&state.pool, id).await?;
    groups::require_graph_access(&state.pool, existing.graph_id, user.id).await?;
    Ok(Json(
        comments::update(
            &state.pool,
            id,
            user.id,
            comments::NodeCommentPatch {
                body: body.body,
                kind: body.kind,
                visibility: body.visibility,
            },
        )
        .await?,
    ))
}

async fn delete_node_comment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let existing = comments::get(&state.pool, id).await?;
    groups::require_graph_access(&state.pool, existing.graph_id, user.id).await?;
    comments::delete(&state.pool, id, user.id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
