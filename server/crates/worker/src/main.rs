mod arxiv_html;
mod jobs;
mod metadata;
mod neighbors;
mod pdf_render;

use epistemic_core::domain::job_kind;
use epistemic_core::repo::jobs as job_repo;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("epistemic=debug".parse()?))
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://epistemic:epistemic@localhost:5432/epistemic".into());
    let pdf_dir = PathBuf::from(std::env::var("PDF_DIR").unwrap_or_else(|_| "./data/pdfs".into()));
    // Optional: only used to read pre-existing TEI from before GROBID removal.
    let tei_dir = PathBuf::from(std::env::var("TEI_DIR").unwrap_or_else(|_| "./data/tei".into()));
    tokio::fs::create_dir_all(&pdf_dir).await?;

    let pool = epistemic_core::connect_no_migrate(&database_url).await?;
    let worker_id = format!("worker-{}", &Uuid::new_v4().to_string()[..8]);

    // Parallel in-process workers (claim is SKIP LOCKED-safe).
    // Default 16 — user said LLM API has no concurrency limit.
    let concurrency: usize = std::env::var("WORKER_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(16);
    tracing::info!(%worker_id, concurrency, "epistemic-worker starting (parallel, no GROBID)");

    let llm = epistemic_llm::LlmClient::from_env().ok();
    if llm.is_none() {
        tracing::warn!("OPENAI_API_KEY (or LLM_API_KEY) not set — DNA extraction jobs will fail");
    }
    let embed = epistemic_llm::EmbeddingClient::from_env().ok();
    if embed.is_none() {
        tracing::warn!("EMBEDDING_API_KEY not set — embed/topic neighbor jobs will fail");
    } else if let Some(ref e) = embed {
        tracing::info!(model = e.model(), dim = ?e.dimensions(), "embedding client ready");
    }

    let ctx = Arc::new(jobs::JobContext {
        pool: pool.clone(),
        pdf_dir,
        tei_dir,
        llm,
        embed,
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .pool_max_idle_per_host(concurrency.max(4))
            .user_agent("EpistemicWorker/0.1 (research library; contact: admin@example.com)")
            .build()?,
    });

    let max_attempts = 3i32;
    let mut handles = Vec::with_capacity(concurrency);
    for slot in 0..concurrency {
        let pool = pool.clone();
        let ctx = Arc::clone(&ctx);
        let slot_id = format!("{worker_id}-s{slot:02}");
        handles.push(tokio::spawn(async move {
            worker_loop(pool, ctx, slot_id, max_attempts).await;
        }));
    }

    // Never returns under normal operation.
    futures::future::join_all(handles).await;
    Ok(())
}

async fn worker_loop(
    pool: sqlx::PgPool,
    ctx: Arc<jobs::JobContext>,
    slot_id: String,
    max_attempts: i32,
) {
    loop {
        match job_repo::claim_next(&pool, &slot_id).await {
            Ok(Some(job)) => {
                tracing::info!(
                    slot = %slot_id,
                    id = %job.id,
                    kind = %job.kind,
                    attempts = job.attempts,
                    "claimed job"
                );
                let result = jobs::dispatch(ctx.as_ref(), &job).await;
                match result {
                    Ok(()) => {
                        if let Err(e) = job_repo::mark_done(&pool, job.id).await {
                            tracing::error!(error = %e, "mark_done failed");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, kind = %job.kind, slot = %slot_id, "job failed");
                        let _ = job_repo::mark_failed(
                            &pool,
                            job.id,
                            &e.to_string(),
                            job.attempts,
                            max_attempts,
                        )
                        .await;
                    }
                }
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            Err(e) => {
                tracing::error!(error = %e, slot = %slot_id, "claim_next error");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

#[allow(dead_code)]
fn _kinds() {
    let _ = [
        job_kind::RESOLVE_METADATA,
        job_kind::FETCH_PDF,
        job_kind::GROBID_PARSE, // deprecated alias → extract_dna
        job_kind::EXTRACT_DNA,
        job_kind::FETCH_REFERENCES,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        job_kind::UPDATE_NEIGHBORS_LINEAGE,
        job_kind::CLASSIFY_CITATION_CONTEXTS,
        job_kind::EMBED,
        job_kind::PROPOSE_PAIRS,
    ];
}
