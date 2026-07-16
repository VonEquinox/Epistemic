use super::{version_id, JobContext};
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::{jobs, works};

/// Download an arXiv PDF and enqueue VLM extraction after the file is durable.
pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    if let Some(rel) = &version.pdf_path {
        if ctx.pdf_dir.join(rel).exists() {
            jobs::enqueue_unique(
                &ctx.pool,
                job_kind::EXTRACT_DNA,
                serde_json::json!({ "version_id": vid, "work_id": version.work_id }),
                &format!("pipeline:{}:{}", job_kind::EXTRACT_DNA, vid),
            )
            .await?;
            tracing::info!(%vid, "fetch_pdf: PDF already present; extraction ensured");
            return Ok(());
        }
        tracing::warn!(%vid, %rel, "fetch_pdf: stored PDF path is missing; downloading again");
    }
    let arxiv = version
        .arxiv_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no arxiv_id for fetch_pdf"))?;

    let url = format!("https://arxiv.org/pdf/{arxiv}.pdf");
    tracing::info!(%url, "downloading PDF for VLM extraction");
    let resp = ctx.http.get(&url).send().await?;
    if resp.status().as_u16() == 429 {
        anyhow::bail!("PDF rate limited 429 — will retry later");
    }
    let bytes = resp.error_for_status()?.bytes().await?;

    let rel = format!("{vid}/{arxiv}.pdf");
    let dest = ctx.pdf_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&dest, &bytes).await?;
    works::update_version_paths(&ctx.pool, vid, Some(&rel), None).await?;
    jobs::enqueue_unique(
        &ctx.pool,
        job_kind::EXTRACT_DNA,
        serde_json::json!({ "version_id": vid, "work_id": version.work_id }),
        &format!("pipeline:{}:{}", job_kind::EXTRACT_DNA, vid),
    )
    .await?;
    tracing::info!(path = %dest.display(), "PDF saved; extraction enqueued");
    Ok(())
}
