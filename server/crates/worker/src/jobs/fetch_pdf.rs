use super::{version_id, JobContext};
use epistemic_core::domain::Job;
use epistemic_core::repo::works;

/// Optional PDF download for reading UI only.
/// DNA extraction uses arXiv HTML (see resolve/extract) — do not enqueue extract here.
pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    if version.pdf_path.is_some() {
        tracing::info!(%vid, "fetch_pdf: already have pdf_path, skip");
        return Ok(());
    }
    let arxiv = version
        .arxiv_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no arxiv_id for fetch_pdf"))?;

    let url = format!("https://arxiv.org/pdf/{arxiv}.pdf");
    tracing::info!(%url, "downloading PDF (optional; DNA uses HTML)");
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
    tracing::info!(path = %dest.display(), "PDF saved");
    Ok(())
}
