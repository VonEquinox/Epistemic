use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{VersionKind, Work, WorkCard, job_kind};
use epistemic_core::repo::{jobs, works};
use epistemic_core::util::{parse_arxiv_id, parse_doi};
use epistemic_core::AppError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_works))
        .route("/quick-add", post(quick_add))
        .route("/{id}", get(get_work))
        .route("/{id}/merge", post(merge))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub query: Option<String>,
    pub project: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

async fn list_works(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<works::WorkListItem>>> {
    let items = works::list_works(
        &state.pool,
        works::WorkListQuery {
            query: q.query,
            project_id: q.project,
            limit: q.limit.unwrap_or(50),
            offset: q.offset.unwrap_or(0),
        },
    )
    .await?;
    Ok(Json(items))
}

async fn get_work(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<WorkCard>> {
    Ok(Json(works::get_work_card(&state.pool, id).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct QuickAddReq {
    /// arXiv URL/ID or DOI
    pub input: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QuickAddResp {
    pub work: Work,
    pub version_id: Uuid,
    pub created: bool,
}

async fn quick_add(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<QuickAddReq>,
) -> ApiResult<Json<QuickAddResp>> {
    let input = body.input.trim();
    let arxiv_id = parse_arxiv_id(input);
    let doi = parse_doi(input);

    if arxiv_id.is_none() && doi.is_none() {
        return Err(AppError::BadRequest(
            "provide an arXiv id/URL or DOI".into(),
        )
        .into());
    }

    let title = arxiv_id
        .as_ref()
        .map(|id| format!("arXiv:{id}"))
        .or_else(|| doi.as_ref().map(|d| format!("DOI:{d}")))
        .unwrap_or_else(|| input.to_string());

    let kind = if arxiv_id.is_some() {
        VersionKind::Arxiv
    } else {
        VersionKind::Other
    };

    let (work, version, created) = works::create_or_get_work(
        &state.pool,
        works::NewVersion {
            kind,
            arxiv_id: arxiv_id.clone(),
            doi,
            url: Some(input.to_string()),
            title,
            abstract_text: String::new(),
            year: None,
            venue_name: None,
            metadata_source: Some("quick_add".into()),
            author_names: vec![],
        },
        Some(user.id),
    )
    .await?;

    if created {
        // Enqueue pipeline
        let payload = serde_json::json!({
            "version_id": version.id,
            "work_id": work.id,
        });
        jobs::enqueue(&state.pool, job_kind::RESOLVE_METADATA, payload.clone()).await?;
        if arxiv_id.is_some() {
            jobs::enqueue(&state.pool, job_kind::FETCH_PDF, payload.clone()).await?;
        }
        jobs::enqueue(&state.pool, job_kind::FETCH_REFERENCES, payload).await?;
    }

    Ok(Json(QuickAddResp {
        work,
        version_id: version.id,
        created,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergeReq {
    pub merged_work_id: Uuid,
}

async fn merge(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MergeReq>,
) -> ApiResult<Json<Work>> {
    Ok(Json(
        works::merge_works(&state.pool, id, body.merged_work_id, user.id).await?,
    ))
}
