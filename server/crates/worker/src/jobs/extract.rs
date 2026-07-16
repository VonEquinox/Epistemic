use super::{version_id, JobContext};
use crate::arxiv_html;
use crate::metadata::RefItem;
use epistemic_core::domain::{job_kind, ASPECTS, ReviewStatus, SourceLayer, Job};
use epistemic_core::repo::{aspects, jobs, works};
use epistemic_llm::estimate_cost_usd;

const PROMPT_VERSION: &str = "dna_aspects_v1";
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

    // Clear prior AI DNA rows so re-extract is idempotent.
    sqlx::query(
        r#"DELETE FROM claims WHERE work_id = $1 AND source = 'ai_candidate'"#,
    )
    .bind(work_id)
    .execute(&ctx.pool)
    .await?;
    sqlx::query(
        r#"DELETE FROM methods WHERE work_id = $1 AND source = 'ai_candidate'"#,
    )
    .bind(work_id)
    .execute(&ctx.pool)
    .await?;
    aspects::delete_for_work(&ctx.pool, work_id).await?;

    // Upsert fixed 8 aspects + evidence spans.
    for def in ASPECTS {
        let Some(obj) = value.get(def.key) else {
            continue;
        };
        let summary = obj
            .get("summary")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let bullets = obj
            .get("bullets")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let source_text = obj
            .get("source_text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let page = obj.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
        let page = if page < 0 { 0 } else { page };

        aspects::upsert(
            &ctx.pool,
            work_id,
            def.key,
            &summary,
            &bullets,
            &source_text,
            page,
            Some(&model),
            Some(PROMPT_VERSION),
        )
        .await?;

        if !source_text.is_empty() {
            insert_field_evidence(&ctx.pool, vid, def.key, obj).await?;
        }

        // Compat materialization: methods / findings → entity tables.
        match def.key {
            "methods" => {
                materialize_methods_from_aspect(&ctx.pool, work_id, vid, obj, &model_ver).await?;
            }
            "findings" => {
                materialize_claims_from_aspect(&ctx.pool, work_id, vid, obj, &model_ver).await?;
            }
            _ => {}
        }
    }

    // Bibliography → citations
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
        if items.is_empty() {
            tracing::warn!(%work_id, "DNA aspects: empty references array");
        } else {
            store_citations(ctx, work_id, &items).await?;
        }
    } else {
        tracing::warn!(%work_id, "DNA aspects: missing references field");
    }

    let payload = serde_json::json!({
        "version_id": vid,
        "work_id": work_id,
    });
    // Primary path: multi-aspect embed. Optional pair/citation paths retained.
    jobs::enqueue(&ctx.pool, job_kind::EMBED, payload.clone()).await?;
    jobs::enqueue(
        &ctx.pool,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        payload.clone(),
    )
    .await?;
    jobs::enqueue(&ctx.pool, job_kind::PROPOSE_PAIRS, payload).await?;

    tracing::info!(%vid, cost_usd = cost, "DNA multi-aspect extraction done");
    Ok(())
}

async fn materialize_methods_from_aspect(
    pool: &sqlx::PgPool,
    work_id: uuid::Uuid,
    vid: uuid::Uuid,
    obj: &serde_json::Value,
    model_ver: &str,
) -> anyhow::Result<()> {
    let bullets = obj
        .get("bullets")
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = obj
        .get("summary")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    let source_text = obj
        .get("source_text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let page = obj.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
    let page = if page < 0 { 0 } else { page };

    let names: Vec<String> = if bullets.is_empty() {
        if summary.is_empty() {
            vec![]
        } else {
            vec![summary.chars().take(120).collect()]
        }
    } else {
        bullets
            .iter()
            .filter_map(|b| b.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect()
    };

    for name in names {
        let desc = if name.as_str() == summary {
            String::new()
        } else {
            summary.to_string()
        };
        sqlx::query(
            r#"
            INSERT INTO methods (work_id, name, description, source, review_status, model_version)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(work_id)
        .bind(&name)
        .bind(&desc)
        .bind(SourceLayer::AiCandidate)
        .bind(ReviewStatus::Unreviewed)
        .bind(model_ver)
        .execute(pool)
        .await?;

        if !source_text.is_empty() {
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (version_id, page, text, extraction_field)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .bind(format!("method:{name}"))
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn materialize_claims_from_aspect(
    pool: &sqlx::PgPool,
    work_id: uuid::Uuid,
    vid: uuid::Uuid,
    obj: &serde_json::Value,
    model_ver: &str,
) -> anyhow::Result<()> {
    let bullets = obj
        .get("bullets")
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = obj
        .get("summary")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    let source_text = obj
        .get("source_text")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let page = obj.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
    let page = if page < 0 { 0 } else { page };

    let texts: Vec<String> = if bullets.is_empty() {
        if summary.is_empty() {
            vec![]
        } else {
            vec![summary.to_string()]
        }
    } else {
        bullets
            .iter()
            .filter_map(|b| b.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect()
    };

    for text in texts {
        let claim_id: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO claims (work_id, text, source, review_status, model_version)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
        )
        .bind(work_id)
        .bind(&text)
        .bind(SourceLayer::AiCandidate)
        .bind(ReviewStatus::Unreviewed)
        .bind(model_ver)
        .fetch_one(pool)
        .await?;

        if !source_text.is_empty() {
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (
                    claim_id, version_id, page, text, extraction_field
                )
                VALUES ($1, $2, $3, $4, 'claim')
                "#,
            )
            .bind(claim_id)
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn extract_from_html_or_text(
    ctx: &JobContext,
    llm: &epistemic_llm::LlmClient,
    version: &epistemic_core::domain::Version,
) -> anyhow::Result<(serde_json::Value, epistemic_llm::Usage, String)> {
    let system = include_str!("../../../llm/prompts/dna_aspects_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/dna_aspects_schema_v1.json"))?;

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
    let aspect_keys: String = ASPECTS
        .iter()
        .map(|a| a.key)
        .collect::<Vec<_>>()
        .join(", ");
    let user = format!(
        "Paper title: {}\nSource: {source} (full paper text).\n\
         Extract the 8 fixed aspects ({aspect_keys}) + full bibliography.\n\
         page may be 0 when unknown; still provide source_text quotes when possible.\n\
         Respond with JSON only matching the schema.\n\n\
         --- BEGIN PAPER ---\n{paper_text}\n--- END PAPER ---",
        version.title
    );
    tracing::info!(
        vid = %version.id,
        source = %source,
        chars = paper_text.chars().count(),
        model = llm.model(),
        "extracting multi-aspect DNA from full text"
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

    tracing::info!(count = refs.len(), %wid, "storing references");
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
        .get("summary")
        .or_else(|| obj.get("text"))
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
    use epistemic_core::domain::{AspectDef, ASPECTS};

    #[test]
    fn schema_parses_and_has_all_aspects() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../llm/prompts/dna_aspects_schema_v1.json"))
                .unwrap();
        let props = schema.get("properties").unwrap();
        for def in ASPECTS {
            assert!(
                props.get(def.key).is_some(),
                "missing aspect property {}",
                def.key
            );
        }
        assert!(props.get("references").is_some());
    }

    #[test]
    fn aspect_defs_are_stable() {
        assert_eq!(ASPECTS.len(), 8);
        assert!(AspectDef::by_key("methods").is_some());
    }
}
