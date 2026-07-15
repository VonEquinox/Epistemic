use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use epistemic_core::domain::{ImportBatch, ImportStatus, VersionKind, job_kind};
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
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ImportBatch>> {
    Ok(Json(imports::get_batch(&state.pool, id).await?))
}

async fn confirm(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let batch = imports::get_batch(&state.pool, id).await?;
    if batch.status != ImportStatus::Preview {
        return Err(AppError::Conflict("batch already confirmed".into()).into());
    }
    imports::set_status(&state.pool, id, ImportStatus::Processing).await?;

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

        match works::create_or_get_work(
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
            Some(user.id),
        )
        .await
        {
            Ok((work, version, is_new)) => {
                if is_new {
                    created += 1;
                    let payload = serde_json::json!({
                        "version_id": version.id,
                        "work_id": work.id,
                    });
                    let _ = jobs::enqueue(
                        &state.pool,
                        job_kind::RESOLVE_METADATA,
                        payload.clone(),
                    )
                    .await;
                    if line.arxiv_id.is_some() {
                        let _ = jobs::enqueue(&state.pool, job_kind::FETCH_PDF, payload.clone())
                            .await;
                    }
                    let _ = jobs::enqueue(&state.pool, job_kind::FETCH_REFERENCES, payload).await;
                } else {
                    skipped += 1;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, raw = %line.raw, "import line failed");
                skipped += 1;
            }
        }
    }

    imports::set_status(&state.pool, id, ImportStatus::Done).await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "created": created,
        "skipped": skipped,
    })))
}
