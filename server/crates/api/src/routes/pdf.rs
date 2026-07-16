use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use epistemic_core::domain::job_kind;
use epistemic_core::repo::{jobs, works};
use epistemic_core::AppError;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{id}/pdf", get(get_pdf).post(upload_pdf))
        .route("/{id}/evidence", get(list_version_evidence))
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
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{id}.pdf\""),
        )
        .body(body)
        .unwrap())
}

/// Multipart upload: field name `file` (application/pdf).
async fn upload_pdf(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    let version = works::get_version(&state.pool, id).await?;

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = format!("{id}.pdf");

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(AppError::BadRequest(format!("multipart: {e}"))))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name == "pdf" {
            if let Some(fname) = field.file_name().map(|s| s.to_string()) {
                filename = fname;
            }
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError(AppError::BadRequest(format!("read field: {e}"))))?;
            if data.len() < 5 || &data[..5] != b"%PDF-" {
                return Err(AppError::BadRequest("file is not a PDF".into()).into());
            }
            // 100 MB cap
            if data.len() > 100 * 1024 * 1024 {
                return Err(AppError::BadRequest("PDF too large (>100MB)".into()).into());
            }
            file_bytes = Some(data.to_vec());
        }
    }

    let bytes = file_bytes.ok_or_else(|| AppError::BadRequest("missing file field".into()))?;

    let safe_name = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe_name = if safe_name.to_lowercase().ends_with(".pdf") {
        safe_name
    } else {
        format!("{safe_name}.pdf")
    };

    let rel = format!("{id}/{safe_name}");
    let dest = state.pdf_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
    }
    tokio::fs::write(&dest, &bytes)
        .await
        .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;

    works::update_version_paths(&state.pool, id, Some(&rel), None).await?;

    // PDF ready → VLM DNA extraction (full page images).
    let payload = serde_json::json!({
        "version_id": id,
        "work_id": version.work_id,
    });
    let _ = jobs::enqueue(&state.pool, job_kind::EXTRACT_DNA, payload).await;

    tracing::info!(%id, path = %dest.display(), bytes = bytes.len(), "PDF uploaded");

    Ok(Json(serde_json::json!({
        "ok": true,
        "pdf_path": rel,
        "bytes": bytes.len(),
        "version_id": id,
    })))
}

async fn list_version_evidence(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<epistemic_core::domain::EvidenceSpan>>> {
    let _ = works::get_version(&state.pool, id).await?;
    Ok(Json(
        epistemic_core::repo::evidence::list_for_version(&state.pool, id).await?,
    ))
}
