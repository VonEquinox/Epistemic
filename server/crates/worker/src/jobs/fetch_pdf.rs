use super::{version_id, JobContext};
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::{jobs, works};

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let arxiv = version
        .arxiv_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no arxiv_id for fetch_pdf"))?;

    let url = format!("https://arxiv.org/pdf/{arxiv}.pdf");
    tracing::info!(%url, "downloading PDF");
    let bytes = ctx.http.get(&url).send().await?.error_for_status()?.bytes().await?;

    let rel = format!("{vid}/{arxiv}.pdf");
    let dest = ctx.pdf_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&dest, &bytes).await?;
    works::update_version_paths(&ctx.pool, vid, Some(&rel), None).await?;
    tracing::info!(path = %dest.display(), "PDF saved");

    // No GROBID: go straight to DNA (title/abstract until a text extractor is wired).
    let payload = serde_json::json!({
        "version_id": vid,
        "work_id": job.payload.get("work_id"),
    });
    jobs::enqueue(&ctx.pool, job_kind::EXTRACT_DNA, payload).await?;
    Ok(())
}
