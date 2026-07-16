use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use epistemic_core::domain::job_kind;
use epistemic_core::repo::{jobs, works};
use epistemic_core::AppError;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const MAX_PDF_BYTES: usize = 100 * 1024 * 1024;
// Include room for multipart headers while keeping the actual file cap at 100 MiB.
const MAX_MULTIPART_BYTES: usize = MAX_PDF_BYTES + 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/{id}/pdf",
            get(get_pdf)
                .post(upload_pdf)
                .layer(DefaultBodyLimit::max(MAX_MULTIPART_BYTES)),
        )
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

async fn upload_pdf(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    let version = works::get_version(&state.pool, id).await?;
    let mut stored: Option<(String, std::path::PathBuf, usize)> = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(AppError::BadRequest(format!("multipart: {e}"))))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" && name != "pdf" {
            continue;
        }
        if stored.is_some() {
            return Err(AppError::BadRequest("only one PDF file is allowed".into()).into());
        }

        let filename = field
            .file_name()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{id}.pdf"));
        let safe_name = safe_pdf_name(&filename);
        let rel = format!("{id}/{safe_name}");
        let dest = state.pdf_dir.join(&rel);
        let parent = dest
            .parent()
            .ok_or_else(|| AppError::Other(anyhow::anyhow!("invalid PDF destination")))?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;

        let temp = parent.join(format!(".upload-{}.tmp", Uuid::new_v4()));
        let mut output = File::create(&temp)
            .await
            .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
        let mut total = 0usize;
        let mut header_bytes = Vec::with_capacity(5);

        let stream_result: Result<(), ApiError> = async {
            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| ApiError(AppError::BadRequest(format!("read field: {e}"))))?
            {
                total = total.saturating_add(chunk.len());
                if total > MAX_PDF_BYTES {
                    return Err(AppError::BadRequest("PDF too large (>100MB)".into()).into());
                }
                if header_bytes.len() < 5 {
                    let need = 5 - header_bytes.len();
                    header_bytes.extend_from_slice(&chunk[..chunk.len().min(need)]);
                }
                output
                    .write_all(&chunk)
                    .await
                    .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
            }
            output
                .flush()
                .await
                .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
            Ok(())
        }
        .await;

        if let Err(error) = stream_result {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error);
        }
        if header_bytes.as_slice() != b"%PDF-" {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(AppError::BadRequest("file is not a PDF".into()).into());
        }
        tokio::fs::rename(&temp, &dest)
            .await
            .map_err(|e| ApiError(AppError::Other(anyhow::anyhow!(e))))?;
        stored = Some((rel, dest, total));
    }

    let (rel, dest, bytes) =
        stored.ok_or_else(|| AppError::BadRequest("missing file field".into()))?;
    works::update_version_paths(&state.pool, id, Some(&rel), None).await?;

    let payload = serde_json::json!({
        "version_id": id,
        "work_id": version.work_id,
    });
    jobs::enqueue_unique(
        &state.pool,
        job_kind::EXTRACT_DNA,
        payload,
        &format!("pipeline:{}:{}", job_kind::EXTRACT_DNA, id),
    )
    .await?;

    tracing::info!(%id, path = %dest.display(), bytes, "PDF uploaded");

    Ok(Json(serde_json::json!({
        "ok": true,
        "pdf_path": rel,
        "bytes": bytes,
        "version_id": id,
    })))
}

fn safe_pdf_name(filename: &str) -> String {
    let safe = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe = if safe.is_empty() {
        "upload".into()
    } else {
        safe
    };
    if safe.to_lowercase().ends_with(".pdf") {
        safe
    } else {
        format!("{safe}.pdf")
    }
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

#[cfg(test)]
mod tests {
    use super::safe_pdf_name;

    #[test]
    fn sanitizes_pdf_names() {
        assert_eq!(safe_pdf_name("../paper 1.PDF"), ".._paper_1.PDF");
        assert_eq!(safe_pdf_name("notes"), "notes.pdf");
    }
}
