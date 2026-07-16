use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{job_kind, ImportBatch, ImportStatus, VersionKind};
use epistemic_core::repo::{imports, jobs, works};
use epistemic_core::AppError;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_preview))
        .route("/{id}", get(get_batch))
        .route("/{id}/confirm", post(confirm))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportReq {
    pub raw_text: String,
}

async fn create_preview(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<ImportReq>,
) -> ApiResult<Json<ImportBatch>> {
    if body.raw_text.trim().is_empty() {
        return Err(AppError::BadRequest("raw_text is empty".into()).into());
    }
    Ok(Json(
        imports::create_batch(&state.pool, user.id, &body.raw_text).await?,
    ))
}

async fn get_batch(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ImportBatch>> {
    Ok(Json(
        imports::get_batch_for_user(&state.pool, id, user.id).await?,
    ))
}

async fn confirm(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let batch = imports::get_batch_for_user(&state.pool, id, user.id).await?;
    imports::begin_confirm(&state.pool, id, user.id).await?;

    let result = confirm_batch(&state, user.id, &batch).await;
    match result {
        Ok((created, skipped)) => {
            imports::set_status(&state.pool, id, ImportStatus::Done).await?;
            Ok(Json(serde_json::json!({
                "ok": true,
                "created": created,
                "skipped": skipped
            })))
        }
        Err(error) => {
            if let Err(status_error) =
                imports::set_status(&state.pool, id, ImportStatus::Failed).await
            {
                tracing::error!(%id, error = %status_error, "failed to mark import batch failed");
            }
            Err(error.into())
        }
    }
}

async fn confirm_batch(
    state: &AppState,
    user_id: Uuid,
    batch: &ImportBatch,
) -> epistemic_core::AppResult<(u32, u32)> {
    let lines: Vec<imports::ParsedImportLine> = batch
        .parsed
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let mut created = 0u32;
    let mut skipped = 0u32;

    for line in lines {
        if line.arxiv_id.is_none() && line.doi.is_none() {
            skipped += 1;
            continue;
        }
        let title = line
            .title
            .clone()
            .or_else(|| line.arxiv_id.as_ref().map(|id| format!("arXiv:{id}")))
            .or_else(|| line.doi.as_ref().map(|d| format!("DOI:{d}")))
            .unwrap_or_else(|| line.raw.clone());
        let kind = if line.arxiv_id.is_some() {
            VersionKind::Arxiv
        } else {
            VersionKind::Other
        };

        let (work, version, is_new) = works::create_or_get_work(
            &state.pool,
            works::NewVersion {
                kind,
                arxiv_id: line.arxiv_id.clone(),
                doi: line.doi.clone(),
                url: line.url.clone(),
                title,
                abstract_text: String::new(),
                year: None,
                venue_name: None,
                metadata_source: Some("import".into()),
                author_names: vec![],
            },
            Some(user_id),
        )
        .await?;

        if is_new {
            created += 1;
        } else {
            skipped += 1;
        }

        let payload = serde_json::json!({
            "version_id": version.id,
            "work_id": work.id,
        });
        jobs::enqueue_unique(
            &state.pool,
            job_kind::RESOLVE_METADATA,
            payload.clone(),
            &format!("pipeline:{}:{}", job_kind::RESOLVE_METADATA, version.id),
        )
        .await?;
        if line.arxiv_id.is_some() {
            jobs::enqueue_unique(
                &state.pool,
                job_kind::FETCH_PDF,
                payload.clone(),
                &format!("pipeline:{}:{}", job_kind::FETCH_PDF, version.id),
            )
            .await?;
        }
        jobs::enqueue_unique(
            &state.pool,
            job_kind::FETCH_REFERENCES,
            payload,
            &format!("pipeline:{}:{}", job_kind::FETCH_REFERENCES, version.id),
        )
        .await?;
    }

    Ok((created, skipped))
}
