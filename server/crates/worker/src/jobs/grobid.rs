use super::{version_id, JobContext};
use epistemic_core::domain::{job_kind, Job};
use epistemic_core::repo::{jobs, works};

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let pdf_rel = version
        .pdf_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no pdf_path for grobid"))?;
    let pdf_path = ctx.pdf_dir.join(pdf_rel);
    if !pdf_path.exists() {
        anyhow::bail!("PDF not found at {}", pdf_path.display());
    }

    let bytes = tokio::fs::read(&pdf_path).await?;
    let url = format!("{}/api/processFulltextDocument", ctx.grobid_url.trim_end_matches('/'));

    tracing::info!(%url, "calling GROBID");
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("paper.pdf")
        .mime_str("application/pdf")?;
    let form = reqwest::multipart::Form::new()
        .part("input", part)
        .text("teiCoordinates", "persName,figure,ref,biblStruct,formula,s,head,p,note,title");

    let resp = ctx.http.post(&url).multipart(form).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GROBID {status}: {body}");
    }
    let tei = resp.text().await?;

    let tei_rel = format!("{vid}/document.tei.xml");
    let tei_path = ctx.tei_dir.join(&tei_rel);
    if let Some(parent) = tei_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&tei_path, &tei).await?;
    works::update_version_paths(&ctx.pool, vid, None, Some(&tei_rel)).await?;
    tracing::info!(path = %tei_path.display(), "TEI saved");

    // Chain DNA extraction
    let payload = serde_json::json!({
        "version_id": vid,
        "work_id": job.payload.get("work_id"),
    });
    jobs::enqueue(&ctx.pool, job_kind::EXTRACT_DNA, payload).await?;
    Ok(())
}
