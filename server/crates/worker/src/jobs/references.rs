use super::{version_id, work_id, JobContext};
use crate::metadata;
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::{jobs, works};

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let wid = work_id(job).unwrap_or(version.work_id);

    let refs = metadata::fetch_references(
        &ctx.http,
        ctx.s2_api_key.as_deref(),
        version.arxiv_id.as_deref(),
        version.doi.as_deref(),
    )
    .await?;

    tracing::info!(count = refs.len(), %wid, "storing references");
    for r in &refs {
        // Try to link to existing work by arxiv/doi
        let cited_work_id = if let Some(ref ax) = r.arxiv_id {
            works::find_version_by_arxiv(&ctx.pool, ax)
                .await?
                .map(|v| v.work_id)
        } else if let Some(ref doi) = r.doi {
            works::find_version_by_doi(&ctx.pool, doi)
                .await?
                .map(|v| v.work_id)
        } else {
            None
        };

        let external = if cited_work_id.is_none() {
            Some(serde_json::json!({
                "title": r.title,
                "arxiv_id": r.arxiv_id,
                "doi": r.doi,
                "year": r.year,
            }))
        } else {
            None
        };

        sqlx::query(
            r#"
            INSERT INTO citations (citing_work_id, cited_work_id, cited_external)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(wid)
        .bind(cited_work_id)
        .bind(external)
        .execute(&ctx.pool)
        .await?;
    }

    // Update citation coupling neighbors
    jobs::enqueue(
        &ctx.pool,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        serde_json::json!({ "work_id": wid, "version_id": vid }),
    )
    .await?;
    Ok(())
}
