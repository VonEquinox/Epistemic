use super::{version_id, work_id, JobContext};
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::{jobs, works};

/// Bibliography job. GROBID/TEI path removed — no automatic refs unless
/// a future extractor writes citations. Still refreshes citation neighbors.
pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let wid = work_id(job).unwrap_or(version.work_id);

    // Optional: if an old tei_path exists on disk, still parse it (legacy data).
    let mut refs = Vec::new();
    if let Some(ref tei_rel) = version.tei_path {
        let tei_path = ctx.tei_dir.join(tei_rel);
        if tei_path.exists() {
            let tei = tokio::fs::read_to_string(&tei_path).await?;
            refs = crate::metadata::references_from_tei(&tei);
            tracing::info!(count = refs.len(), %vid, "references from legacy TEI");
        }
    }

    if refs.is_empty() {
        tracing::info!(
            %vid,
            "fetch_references: no bibliography source (GROBID removed); citations unchanged"
        );
        jobs::enqueue(
            &ctx.pool,
            job_kind::UPDATE_NEIGHBORS_CITATION,
            serde_json::json!({ "work_id": wid, "version_id": vid }),
        )
        .await?;
        return Ok(());
    }

    sqlx::query(r#"DELETE FROM citations WHERE citing_work_id = $1"#)
        .bind(wid)
        .execute(&ctx.pool)
        .await?;

    tracing::info!(count = refs.len(), %wid, "storing references");
    for r in &refs {
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

    jobs::enqueue(
        &ctx.pool,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        serde_json::json!({ "work_id": wid, "version_id": vid }),
    )
    .await?;
    Ok(())
}
