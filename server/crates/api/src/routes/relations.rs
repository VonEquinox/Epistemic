use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{
    EntityKind, MemberRole, RelationDetail, RelationType, ReviewStatus, ReviewVerdict, SourceLayer,
};
use epistemic_core::repo::relations;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create))
        .route("/{id}", get(get_one).patch(patch_relation_handler))
        .route("/{id}/review", post(review))
}

// also expose review-queue at top level via collab

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRelationReq {
    pub relation_type: RelationType,
    pub aspect: Option<String>,
    pub scope: Option<String>,
    pub explanation: Option<String>,
    pub source_work_id: Uuid,
    pub target_work_id: Uuid,
    pub confidence: Option<f64>,
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateRelationReq>,
) -> ApiResult<Json<RelationDetail>> {
    let detail = relations::create_relation(
        &state.pool,
        relations::NewRelation {
            relation_type: body.relation_type,
            aspect: body.aspect,
            scope: body.scope,
            explanation: body.explanation.unwrap_or_default(),
            confidence: body.confidence,
            source: SourceLayer::TeamRecord,
            review_status: ReviewStatus::Confirmed,
            created_by_user: Some(user.id),
            model_version: None,
            members: vec![
                relations::NewRelationMember {
                    entity_kind: EntityKind::Work,
                    entity_id: body.source_work_id,
                    role: MemberRole::Source,
                    anchor_work_id: Some(body.source_work_id),
                    position: 0,
                },
                relations::NewRelationMember {
                    entity_kind: EntityKind::Work,
                    entity_id: body.target_work_id,
                    role: MemberRole::Target,
                    anchor_work_id: Some(body.target_work_id),
                    position: 1,
                },
            ],
            evidence: vec![],
        },
    )
    .await?;
    Ok(Json(detail))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<RelationDetail>> {
    Ok(Json(relations::get_relation(&state.pool, id).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchRelationReq {
    pub relation_type: Option<RelationType>,
    pub aspect: Option<String>,
    pub explanation: Option<String>,
    pub swap_direction: Option<bool>,
    pub review_status: Option<ReviewStatus>,
}

async fn patch_relation_handler(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchRelationReq>,
) -> ApiResult<Json<RelationDetail>> {
    if let Some(status) = body.review_status {
        let d = relations::set_review_status(&state.pool, id, status, user.id).await?;
        // still apply other patches
        if body.relation_type.is_some()
            || body.aspect.is_some()
            || body.explanation.is_some()
            || body.swap_direction.unwrap_or(false)
        {
            return Ok(Json(
                relations::patch_relation(
                    &state.pool,
                    id,
                    body.relation_type,
                    body.aspect,
                    body.explanation,
                    body.swap_direction.unwrap_or(false),
                )
                .await?,
            ));
        }
        return Ok(Json(d));
    }
    Ok(Json(
        relations::patch_relation(
            &state.pool,
            id,
            body.relation_type,
            body.aspect,
            body.explanation,
            body.swap_direction.unwrap_or(false),
        )
        .await?,
    ))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReviewReq {
    pub verdict: ReviewVerdict,
    pub comment: Option<String>,
}

async fn review(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReviewReq>,
) -> ApiResult<Json<RelationDetail>> {
    Ok(Json(
        relations::add_review(
            &state.pool,
            id,
            user.id,
            body.verdict,
            body.comment.as_deref().unwrap_or(""),
        )
        .await?,
    ))
}

// re-export queue handler for collab module convenience
#[derive(Debug, Deserialize)]
pub struct QueueQuery {
    pub work: Option<Uuid>,
    pub all: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn review_queue(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Query(q): Query<QueueQuery>,
) -> ApiResult<Json<Vec<RelationDetail>>> {
    Ok(Json(
        relations::review_queue(
            &state.pool,
            relations::ReviewQueueQuery {
                work_id: q.work,
                only_unreviewed: !q.all.unwrap_or(false),
                limit: q.limit.unwrap_or(50),
                offset: q.offset.unwrap_or(0),
            },
        )
        .await?,
    ))
}
