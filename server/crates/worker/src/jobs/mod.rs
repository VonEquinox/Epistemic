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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Done,
    Rescheduled,
}

pub async fn dispatch(ctx: &JobContext, job: &Job, worker_id: &str) -> anyhow::Result<JobOutcome> {
    match job.kind.as_str() {
        job_kind::RESOLVE_METADATA => resolve::run(ctx, job).await.map(|_| JobOutcome::Done),
        job_kind::FETCH_PDF => fetch_pdf::run(ctx, job).await.map(|_| JobOutcome::Done),
        job_kind::GROBID_PARSE => {
            tracing::warn!(id = %job.id, "grobid_parse deprecated — running extract_dna instead");
            extract::run(ctx, job).await.map(|_| JobOutcome::Done)
        }
        job_kind::EXTRACT_DNA => extract::run(ctx, job).await.map(|_| JobOutcome::Done),
        job_kind::FETCH_REFERENCES => references::run(ctx, job).await.map(|_| JobOutcome::Done),
        job_kind::UPDATE_NEIGHBORS_CITATION => neighbors::update_citation(ctx, job)
            .await
            .map(|_| JobOutcome::Done),
        job_kind::UPDATE_NEIGHBORS_LINEAGE => neighbors::update_lineage(ctx, job)
            .await
            .map(|_| JobOutcome::Done),
        job_kind::CLASSIFY_CITATION_CONTEXTS => {
            classify_cite::run(ctx, job).await.map(|_| JobOutcome::Done)
        }
        job_kind::PROPOSE_PAIRS => propose::run(ctx, job).await.map(|_| JobOutcome::Done),
        job_kind::BATCH_ORCH => batch_orch::run(ctx, job, worker_id).await,
        job_kind::EMBED => embed::run(ctx, job).await.map(|_| JobOutcome::Done),
        other => anyhow::bail!("unknown job kind: {other}"),
    }
}

pub fn version_id(job: &Job) -> anyhow::Result<uuid::Uuid> {
    job.payload
        .get("version_id")
        .and_then(|value| value.as_str())
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or_else(|| anyhow::anyhow!("payload.version_id missing"))
}

pub fn work_id(job: &Job) -> Option<uuid::Uuid> {
    job.payload
        .get("work_id")
        .and_then(|value| value.as_str())
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
}
