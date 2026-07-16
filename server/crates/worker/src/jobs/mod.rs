mod batch_orch;
mod classify_cite;
mod embed;
mod extract;
mod fetch_pdf;
mod propose;
mod references;
mod resolve;

use epistemic_core::domain::{job_kind, Job};
use sqlx::PgPool;
use std::path::PathBuf;

use crate::neighbors;

pub struct JobContext {
    pub pool: PgPool,
    pub pdf_dir: PathBuf,
    /// Legacy TEI directory (optional reads if old tei_path rows exist). No writer.
    pub tei_dir: PathBuf,
    pub llm: Option<epistemic_llm::LlmClient>,
    pub embed: Option<epistemic_llm::EmbeddingClient>,
    pub http: reqwest::Client,
}

pub async fn dispatch(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    match job.kind.as_str() {
        job_kind::RESOLVE_METADATA => resolve::run(ctx, job).await,
        job_kind::FETCH_PDF => fetch_pdf::run(ctx, job).await,
        // GROBID removed: legacy job kind falls through to VLM DNA extraction.
        job_kind::GROBID_PARSE => {
            tracing::warn!(id = %job.id, "grobid_parse deprecated — running extract_dna (VLM) instead");
            extract::run(ctx, job).await
        }
        job_kind::EXTRACT_DNA => extract::run(ctx, job).await,
        job_kind::FETCH_REFERENCES => references::run(ctx, job).await,
        job_kind::UPDATE_NEIGHBORS_CITATION => neighbors::update_citation(ctx, job).await,
        job_kind::UPDATE_NEIGHBORS_LINEAGE => neighbors::update_lineage(ctx, job).await,
        job_kind::CLASSIFY_CITATION_CONTEXTS => classify_cite::run(ctx, job).await,
        job_kind::PROPOSE_PAIRS => propose::run(ctx, job).await,
        job_kind::BATCH_ORCH => batch_orch::run(ctx, job).await,
        job_kind::EMBED => embed::run(ctx, job).await,
        other => {
            tracing::warn!(kind = other, "unknown job kind, marking done");
            Ok(())
        }
    }
}

pub fn version_id(job: &Job) -> anyhow::Result<uuid::Uuid> {
    job.payload
        .get("version_id")
        .and_then(|v| v.as_str())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow::anyhow!("payload.version_id missing"))
}

pub fn work_id(job: &Job) -> Option<uuid::Uuid> {
    job.payload
        .get("work_id")
        .and_then(|v| v.as_str())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
}
