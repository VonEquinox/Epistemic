use super::{version_id, JobContext};
use crate::arxiv_html;
use epistemic_core::domain::Job;
use epistemic_core::repo::works;

/// Resolve title/abstract/authors from arXiv HTML experimental (no export.arxiv.org API).
pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;

    let Some(ref arxiv) = version.arxiv_id else {
        tracing::warn!(%vid, "resolve_metadata: no arxiv_id");
        return Ok(());
    };

    tracing::info!(%arxiv, "resolving metadata from arXiv HTML");
    let doc = arxiv_html::fetch_arxiv_html(&ctx.http, arxiv).await?;

    // Persist raw HTML next to PDFs for audit / re-extract.
    let rel = format!("{vid}/{arxiv}.html");
    let dest = ctx.pdf_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&dest, doc.html.as_bytes()).await?;
    // Reuse tei_path column to store html path (GROBID TEI gone).
    works::update_version_paths(&ctx.pool, vid, None, Some(&rel)).await?;

    let title = doc
        .title
        .filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("arxiv"))
        .unwrap_or_else(|| version.title.clone());
    let abstract_text = doc.abstract_text.unwrap_or_else(|| {
        // First ~1500 chars of body as weak abstract if missing.
        doc.full_text.chars().take(1500).collect()
    });

    works::update_version_metadata(
        &ctx.pool,
        vid,
        Some(&title),
        Some(&abstract_text),
        None,
        Some("arXiv"),
        None,
        Some("arxiv_html"),
    )
    .await?;

    if !doc.authors.is_empty() {
        let existing = works::authors_for_version(&ctx.pool, vid).await?;
        if existing.is_empty() {
            for (pos, name) in doc.authors.iter().enumerate() {
                let author_id: uuid::Uuid = sqlx::query_scalar(
                    r#"
                    INSERT INTO authors (full_name)
                    VALUES ($1)
                    ON CONFLICT DO NOTHING
                    RETURNING id
                    "#,
                )
                .bind(name)
                .fetch_optional(&ctx.pool)
                .await?
                .unwrap_or(
                    sqlx::query_scalar(r#"SELECT id FROM authors WHERE full_name = $1 LIMIT 1"#)
                        .bind(name)
                        .fetch_one(&ctx.pool)
                        .await?,
                );
                sqlx::query(
                    r#"
                    INSERT INTO version_authors (version_id, author_id, position)
                    VALUES ($1, $2, $3) ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(vid)
                .bind(author_id)
                .bind(pos as i32)
                .execute(&ctx.pool)
                .await?;
            }
        }
    }

    tracing::info!(
        %arxiv,
        title = %title,
        text_chars = doc.full_text.chars().count(),
        "HTML metadata resolved; waiting for PDF fetch/upload"
    );
    Ok(())
}
