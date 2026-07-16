use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use epistemic_core::domain::{Graph, GraphWithMeta, GroupRole, ResearchGroup, ResearchGroupWithMeta};
use epistemic_core::repo::groups;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_groups).post(create_group))
        .route("/{id}", get(get_group))
        .route("/{id}/members", get(list_members).post(add_member))
        .route("/{id}/graphs", get(list_graphs).post(create_graph))
        .route("/graphs/{graph_id}", get(get_graph))
        .route(
            "/graphs/{graph_id}/works",
            post(add_works).delete(remove_work_query),
        )
        .route(
            "/graphs/{graph_id}/works/{work_id}",
            delete(remove_work).post(add_one_work),
        )
        .route("/graphs/{graph_id}/import-library", post(import_library))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupReq {
    pub name: String,
    pub description: Option<String>,
}

async fn list_groups(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<ResearchGroupWithMeta>>> {
    Ok(Json(groups::list_for_user(&state.pool, user.id).await?))
}

async fn create_group(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateGroupReq>,
) -> ApiResult<Json<ResearchGroup>> {
    Ok(Json(
        groups::create_group(
            &state.pool,
            &body.name,
            body.description.as_deref().unwrap_or(""),
            user.id,
        )
        .await?,
    ))
}

async fn get_group(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ResearchGroupWithMeta>> {
    groups::require_member(&state.pool, id, user.id).await?;
    let g = groups::get_group(&state.pool, id).await?;
    let members = groups::list_members(&state.pool, id).await?;
    let graphs = groups::list_graphs(&state.pool, id).await?;
    let my_role = members
        .iter()
        .find(|m| m.user_id == user.id)
        .map(|m| m.role)
        .unwrap_or(GroupRole::Member);
    Ok(Json(ResearchGroupWithMeta {
        group: g,
        my_role,
        member_count: members.len() as i64,
        graph_count: graphs.len() as i64,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMemberReq {
    pub user_id: Uuid,
    pub role: Option<GroupRole>,
}

async fn list_members(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<groups::MemberPublic>>> {
    groups::require_member(&state.pool, id, user.id).await?;
    Ok(Json(groups::list_members_public(&state.pool, id).await?))
}

async fn add_member(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMemberReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let role = groups::require_member(&state.pool, id, user.id).await?;
    if !matches!(role, GroupRole::Owner | GroupRole::Admin) {
        return Err(epistemic_core::error::AppError::Forbidden(
            "only owner/admin can add members".into(),
        )
        .into());
    }
    groups::add_member(
        &state.pool,
        id,
        body.user_id,
        body.role.unwrap_or(GroupRole::Member),
    )
    .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGraphReq {
    pub name: String,
    pub description: Option<String>,
}

async fn list_graphs(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<GraphWithMeta>>> {
    groups::require_member(&state.pool, id, user.id).await?;
    Ok(Json(groups::list_graphs(&state.pool, id).await?))
}

async fn create_graph(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateGraphReq>,
) -> ApiResult<Json<Graph>> {
    groups::require_member(&state.pool, id, user.id).await?;
    Ok(Json(
        groups::create_graph(
            &state.pool,
            id,
            &body.name,
            body.description.as_deref().unwrap_or(""),
            user.id,
        )
        .await?,
    ))
}

async fn get_graph(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(graph_id): Path<Uuid>,
) -> ApiResult<Json<GraphWithMeta>> {
    let g = groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    let n = groups::list_graph_work_ids(&state.pool, graph_id)
        .await?
        .len() as i64;
    Ok(Json(GraphWithMeta {
        graph: g,
        work_count: n,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddWorksReq {
    pub work_ids: Vec<Uuid>,
}

async fn add_works(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(graph_id): Path<Uuid>,
    Json(body): Json<AddWorksReq>,
) -> ApiResult<Json<serde_json::Value>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    let n = groups::add_works(&state.pool, graph_id, &body.work_ids, user.id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "added": n })))
}

async fn add_one_work(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((graph_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    let n = groups::add_works(&state.pool, graph_id, &[work_id], user.id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "added": n })))
}

async fn remove_work(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((graph_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    groups::remove_work(&state.pool, graph_id, work_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct RemoveWorkQuery {
    pub work_id: Uuid,
}

async fn remove_work_query(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(graph_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<RemoveWorkQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    groups::remove_work(&state.pool, graph_id, q.work_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn import_library(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(graph_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    groups::require_graph_access(&state.pool, graph_id, user.id).await?;
    let n = groups::add_all_library_works(&state.pool, graph_id, user.id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "added": n })))
}
