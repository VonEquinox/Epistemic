mod jobs;
mod metadata;
mod neighbors;

use epistemic_core::domain::job_kind;
use epistemic_core::repo::jobs as job_repo;
use std::path::PathBuf;
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
    let tei_dir = PathBuf::from(std::env::var("TEI_DIR").unwrap_or_else(|_| "./data/tei".into()));
    let grobid_url =
        std::env::var("GROBID_URL").unwrap_or_else(|_| "http://localhost:8070".into());
    let s2_api_key = std::env::var("S2_API_KEY").ok();
    tokio::fs::create_dir_all(&pdf_dir).await?;
    tokio::fs::create_dir_all(&tei_dir).await?;

    let pool = epistemic_core::connect_no_migrate(&database_url).await?;
    let worker_id = format!("worker-{}", &Uuid::new_v4().to_string()[..8]);
    tracing::info!(%worker_id, "epistemic-worker starting");

    let llm = epistemic_llm::ClaudeClient::from_env().ok();
    if llm.is_none() {
        tracing::warn!("ANTHROPIC_API_KEY not set — DNA extraction jobs will fail");
    }

    let ctx = jobs::JobContext {
        pool: pool.clone(),
        pdf_dir,
        tei_dir,
        grobid_url,
        s2_api_key,
        llm,
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("EpistemicWorker/0.1")
            .build()?,
    };

    let max_attempts = 3i32;
    loop {
        match job_repo::claim_next(&pool, &worker_id).await {
            Ok(Some(job)) => {
                tracing::info!(id = %job.id, kind = %job.kind, attempts = job.attempts, "claimed job");
                let result = jobs::dispatch(&ctx, &job).await;
                match result {
                    Ok(()) => {
                        if let Err(e) = job_repo::mark_done(&pool, job.id).await {
                            tracing::error!(error = %e, "mark_done failed");
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, kind = %job.kind, "job failed");
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
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                tracing::error!(error = %e, "claim_next error");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

// re-export kind constants for match
#[allow(dead_code)]
fn _kinds() {
    let _ = [
        job_kind::RESOLVE_METADATA,
        job_kind::FETCH_PDF,
        job_kind::GROBID_PARSE,
        job_kind::EXTRACT_DNA,
        job_kind::FETCH_REFERENCES,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        job_kind::UPDATE_NEIGHBORS_LINEAGE,
        job_kind::CLASSIFY_CITATION_CONTEXTS,
        job_kind::EMBED,
        job_kind::PROPOSE_PAIRS,
    ];
}
