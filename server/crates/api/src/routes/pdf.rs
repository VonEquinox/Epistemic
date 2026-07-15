use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use epistemic_core::repo::works;
use epistemic_core::AppError;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{id}/pdf", get(get_pdf).post(upload_pdf_meta))
}

async fn get_pdf(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let version = works::get_version(&state.pool, id).await?;
    let rel = version
        .pdf_path
        .ok_or_else(|| AppError::NotFound("no PDF for this version".into()))?;
    let path = state.pdf_dir.join(&rel);
    if !path.exists() {
        return Err(AppError::NotFound(format!("PDF file missing: {rel}")).into());
    }
    let file = File::open(&path)
        .await
        .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{id}.pdf\""),
        )
        .body(body)
        .unwrap())
}

/// Placeholder for multipart upload — records expected path; full multipart in later pass.
#[derive(Debug, serde::Deserialize)]
pub struct UploadMeta {
    pub filename: Option<String>,
}

async fn upload_pdf_meta(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UploadMeta>,
) -> ApiResult<Json<serde_json::Value>> {
    let _ = works::get_version(&state.pool, id).await?;
    let name = body
        .filename
        .unwrap_or_else(|| format!("{id}.pdf"));
    let rel = format!("{id}/{name}");
    // Ensure directory exists
    let dir = state.pdf_dir.join(id.to_string());
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
    works::update_version_paths(&state.pool, id, Some(&rel), None).await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "pdf_path": rel,
        "note": "metadata recorded; place file at pdf_dir path or use multipart endpoint later"
    })))
}
