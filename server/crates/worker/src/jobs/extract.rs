use super::{version_id, JobContext};
use crate::arxiv_html;
use crate::metadata::RefItem;
use epistemic_core::domain::{job_kind, ReviewStatus, SourceLayer, Job};
use epistemic_core::repo::{jobs, works};
use epistemic_llm::estimate_cost_usd;

const PROMPT_VERSION: &str = "dna_html_v1";
/// Keep full text within a safe context window for chat completions.
const MAX_PAPER_CHARS: usize = 120_000;

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let work_id = version.work_id;

    // Prefer full arXiv HTML text → LLM (no export.arxiv.org API, no PDF page VLM).
    let (value, usage, model) = extract_from_html_or_text(ctx, llm, &version).await?;

    let cost = estimate_cost_usd(&model, &usage);

    sqlx::query(
        r#"
        INSERT INTO extractions (version_id, model, prompt_version, raw, status, usage, cost_usd)
        VALUES ($1, $2, $3, $4, 'done', $5, $6)
        "#,
    )
    .bind(vid)
    .bind(&model)
    .bind(PROMPT_VERSION)
    .bind(&value)
    .bind(serde_json::to_value(&usage)?)
    .bind(cost)
    .execute(&ctx.pool)
    .await?;

    let model_ver = format!("{model}/{PROMPT_VERSION}");

    for field in ["research_question"] {
        if let Some(obj) = value.get(field) {
            insert_field_evidence(&ctx.pool, vid, field, obj).await?;
        }
    }
    for field in ["contributions", "limitations", "datasets"] {
        if let Some(arr) = value.get(field).and_then(|v| v.as_array()) {
            for item in arr {
                insert_field_evidence(&ctx.pool, vid, field, item).await?;
            }
        }
    }

    if let Some(claims) = value.get("claims").and_then(|c| c.as_array()) {
        for c in claims {
            let text = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = c.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = c.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            if text.is_empty() || source_text.is_empty() {
                continue;
            }
            let page = if page < 0 { 0 } else { page };
            let claim_id: uuid::Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO claims (work_id, text, source, review_status, model_version)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING id
                "#,
            )
            .bind(work_id)
            .bind(text)
            .bind(SourceLayer::AiCandidate)
            .bind(ReviewStatus::Unreviewed)
            .bind(&model_ver)
            .fetch_one(&ctx.pool)
            .await?;

            let bbox = c.get("bbox").cloned();
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (
                    claim_id, version_id, page, text, extraction_field, bbox
                )
                VALUES ($1, $2, $3, $4, 'claim', $5)
                "#,
            )
            .bind(claim_id)
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .bind(bbox)
            .execute(&ctx.pool)
            .await?;
        }
    }

    if let Some(methods) = value.get("methods").and_then(|m| m.as_array()) {
        for m in methods {
            let name = m.get("name").and_then(|t| t.as_str()).unwrap_or("");
            let desc = m.get("description").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = m.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = m.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            if name.is_empty() || source_text.is_empty() {
                continue;
            }
            let page = if page < 0 { 0 } else { page };
            sqlx::query_scalar::<_, uuid::Uuid>(
                r#"
                INSERT INTO methods (work_id, name, description, source, review_status, model_version)
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id
                "#,
            )
            .bind(work_id)
            .bind(name)
            .bind(desc)
            .bind(SourceLayer::AiCandidate)
            .bind(ReviewStatus::Unreviewed)
            .bind(&model_ver)
            .fetch_one(&ctx.pool)
            .await?;

            let bbox = m.get("bbox").cloned();
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (
                    version_id, page, text, extraction_field, bbox
                )
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .bind(format!("method:{name}"))
            .bind(bbox)
            .execute(&ctx.pool)
            .await?;
        }
    }

    // Bibliography from VLM → citations
    if let Some(refs) = value.get("references").and_then(|r| r.as_array()) {
        let items: Vec<RefItem> = refs
            .iter()
            .filter_map(|r| {
                let title = r
                    .get("title")
                    .and_then(|t| t.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let arxiv_id = r
                    .get("arxiv_id")
                    .and_then(|t| t.as_str())
                    .map(|s| s.trim().trim_start_matches("arXiv:").to_string())
                    .filter(|s| !s.is_empty());
                let doi = r
                    .get("doi")
                    .and_then(|t| t.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let year = r.get("year").and_then(|y| y.as_i64()).map(|y| y as i32);
                if title.is_none() && arxiv_id.is_none() && doi.is_none() {
                    return None;
                }
                Some(RefItem {
                    title,
                    arxiv_id,
                    doi,
                    year,
                })
            })
            .collect();
        if !items.is_empty() {
            store_citations(ctx, work_id, &items).await?;
        }
    }

    let payload = serde_json::json!({
        "version_id": vid,
        "work_id": work_id,
    });
    // Skip classify_cite without TEI; go straight to pairing + embed stub.
    jobs::enqueue(&ctx.pool, job_kind::PROPOSE_PAIRS, payload.clone()).await?;
    jobs::enqueue(
        &ctx.pool,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        payload.clone(),
    )
    .await?;
    jobs::enqueue(&ctx.pool, job_kind::EMBED, payload).await?;

    tracing::info!(%vid, cost_usd = cost, "DNA VLM extraction done");
    Ok(())
}

async fn extract_from_html_or_text(
    ctx: &JobContext,
    llm: &epistemic_llm::LlmClient,
    version: &epistemic_core::domain::Version,
) -> anyhow::Result<(serde_json::Value, epistemic_llm::Usage, String)> {
    let system = include_str!("../../../llm/prompts/dna_vlm_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/dna_vlm_schema_v1.json"))?;

    // 1) Saved HTML on disk (tei_path reused for *.html)
    let mut full = None;
    if let Some(ref rel) = version.tei_path {
        if rel.ends_with(".html") {
            let path = ctx.pdf_dir.join(rel);
            if path.exists() {
                let html = tokio::fs::read_to_string(&path).await?;
                let text = arxiv_html::html_to_text(&html);
                if text.chars().count() > 200 {
                    full = Some((text, format!("file:{rel}")));
                }
            }
        }
    }

    // 2) Live fetch arXiv HTML
    if full.is_none() {
        if let Some(ref arxiv) = version.arxiv_id {
            match arxiv_html::fetch_arxiv_html(&ctx.http, arxiv).await {
                Ok(doc) => {
                    let rel = format!("{}/{arxiv}.html", version.id);
                    let dest = ctx.pdf_dir.join(&rel);
                    if let Some(parent) = dest.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&dest, doc.html.as_bytes()).await?;
                    works::update_version_paths(&ctx.pool, version.id, None, Some(&rel)).await?;
                    full = Some((doc.full_text, doc.url));
                }
                Err(e) => tracing::warn!(%arxiv, error = %e, "HTML fetch failed"),
            }
        }
    }

    let (paper_text, source) = if let Some((text, src)) = full {
        (text, src)
    } else {
        tracing::warn!(vid = %version.id, "no HTML; falling back to title+abstract only");
        (
            format!("{}\n\n{}", version.title, version.abstract_text),
            "title_abstract".into(),
        )
    };

    let paper_text: String = paper_text.chars().take(MAX_PAPER_CHARS).collect();
    let user = format!(
        "Paper title: {}\nSource: {source} (arXiv HTML full text, not page images).\n\
         page fields may be 0 when page is unknown — still provide source_text quotes from the body.\n\
         Extract Paper DNA + full bibliography from the COMPLETE paper text below.\n\
         Respond with JSON only matching the schema.\n\n\
         --- BEGIN PAPER ---\n{paper_text}\n--- END PAPER ---",
        version.title
    );
    tracing::info!(
        vid = %version.id,
        source = %source,
        chars = paper_text.chars().count(),
        model = llm.model(),
        "extracting DNA from arXiv HTML full text"
    );
    Ok(llm.complete_json(system, &user, schema, 16_000).await?)
}

async fn store_citations(
    ctx: &JobContext,
    wid: uuid::Uuid,
    refs: &[RefItem],
) -> anyhow::Result<()> {
    sqlx::query(r#"DELETE FROM citations WHERE citing_work_id = $1"#)
        .bind(wid)
        .execute(&ctx.pool)
        .await?;

    tracing::info!(count = refs.len(), %wid, "storing VLM references");
    for r in refs {
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
    Ok(())
}

async fn insert_field_evidence(
    pool: &sqlx::PgPool,
    version_id: uuid::Uuid,
    field: &str,
    obj: &serde_json::Value,
) -> anyhow::Result<()> {
    let source_text = obj
        .get("source_text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let page = obj.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
    if source_text.is_empty() {
        return Ok(());
    }
    let page = if page < 0 { 0 } else { page };
    let text = obj
        .get("text")
        .or_else(|| obj.get("name"))
        .and_then(|t| t.as_str())
        .unwrap_or(source_text);
    let bbox = obj.get("bbox").cloned();
    sqlx::query(
        r#"
        INSERT INTO evidence_spans (version_id, page, text, extraction_field, bbox)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(version_id)
    .bind(page)
    .bind(text)
    .bind(field)
    .bind(bbox)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_parses() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../llm/prompts/dna_vlm_schema_v1.json"))
                .unwrap();
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"].get("references").is_some());
    }
}
