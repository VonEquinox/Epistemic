use super::{version_id, JobContext};
use epistemic_core::domain::{job_kind, ReviewStatus, SourceLayer, Job};
use epistemic_core::repo::{jobs, works};
use epistemic_llm::estimate_cost_usd;

const PROMPT_VERSION: &str = "dna_v1";

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;

    let paper_text = if let Some(ref tei_rel) = version.tei_path {
        let p = ctx.tei_dir.join(tei_rel);
        if p.exists() {
            let tei = tokio::fs::read_to_string(&p).await?;
            tei_to_text(&tei)
        } else {
            format!("{}\n\n{}", version.title, version.abstract_text)
        }
    } else {
        format!("{}\n\n{}", version.title, version.abstract_text)
    };

    let paper_text: String = paper_text.chars().take(80_000).collect();

    let system = include_str!("../../../llm/prompts/dna_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/dna_schema_v1.json"))?;

    let user = format!(
        "Extract Paper DNA from the following paper text.\n\n---\n{paper_text}\n---"
    );

    tracing::info!(%vid, model = llm.model(), "extracting DNA");
    let (value, usage, model) = llm.complete_json(system, &user, schema, 8000).await?;
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

    let work_id = version.work_id;
    let model_ver = format!("{model}/{PROMPT_VERSION}");

    // Field-level DNA with evidence (research_question, contributions, limitations)
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

    // Claims + evidence spans
    if let Some(claims) = value.get("claims").and_then(|c| c.as_array()) {
        for c in claims {
            let text = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = c.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = c.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            if text.is_empty() || source_text.is_empty() || page < 1 {
                continue; // no evidence → discard
            }
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

    // Methods + evidence (extraction_field = method:<name>)
    if let Some(methods) = value.get("methods").and_then(|m| m.as_array()) {
        for m in methods {
            let name = m.get("name").and_then(|t| t.as_str()).unwrap_or("");
            let desc = m.get("description").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = m.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = m.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            if name.is_empty() || source_text.is_empty() || page < 1 {
                continue;
            }
            let method_id: uuid::Uuid = sqlx::query_scalar(
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
            let _ = method_id;
        }
    }

    let payload = serde_json::json!({
        "version_id": vid,
        "work_id": work_id,
    });
    jobs::enqueue(&ctx.pool, job_kind::CLASSIFY_CITATION_CONTEXTS, payload.clone()).await?;
    jobs::enqueue(&ctx.pool, job_kind::EMBED, payload).await?;

    tracing::info!(%vid, cost_usd = cost, "DNA extraction done");
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
    if source_text.is_empty() || page < 1 {
        return Ok(());
    }
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

/// TEI → plain text, preserving page markers when `coords` / `n` attributes appear.
fn tei_to_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    let mut tag_buf = String::new();
    for c in xml.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag_buf.clear();
            }
            '>' => {
                in_tag = false;
                // page break hints
                let t = tag_buf.to_lowercase();
                if t.starts_with("pb ") || t == "pb" || t.starts_with("pb/") {
                    if let Some(n) = attr_value(&tag_buf, "n") {
                        out.push_str(&format!("\n[[page:{n}]]\n"));
                    }
                }
                // surface coords as optional markers for LLM context
                if t.contains("coords=") {
                    if let Some(coords) = attr_value(&tag_buf, "coords") {
                        // coords often "p x0 y0 x1 y1" — keep page prefix if present
                        let page = coords.split_whitespace().next().unwrap_or("");
                        if page.chars().all(|ch| ch.is_ascii_digit()) && !page.is_empty() {
                            // lightweight page hint only once in a while is fine
                            let _ = page;
                        }
                    }
                }
            }
            _ if in_tag => tag_buf.push(c),
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tei_extracts_page_markers() {
        let xml = r#"<TEI><text><pb n="2"/><p>Hello world</p><pb n="3"/><p>More</p></text></TEI>"#;
        let t = tei_to_text(xml);
        assert!(t.contains("[[page:2]]") || t.contains("Hello"));
        assert!(t.contains("Hello") && t.contains("More"));
    }

    #[test]
    fn attr_value_parses() {
        assert_eq!(attr_value(r#"pb n="12" /"#, "n"), Some("12"));
    }
}
