use super::{version_id, JobContext};
use epistemic_core::domain::Job;
use epistemic_core::repo::works;

/// Bibliography comes from HTML DNA extract. This job is intentionally a no-op
/// (kept so existing queued rows complete without calling arXiv APIs).
pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let _ = works::get_version(&ctx.pool, vid).await?;
    tracing::info!(
        %vid,
        "fetch_references: no-op (refs from arXiv HTML DNA)"
    );
    Ok(())
}
