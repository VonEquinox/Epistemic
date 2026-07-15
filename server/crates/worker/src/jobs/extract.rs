use super::{version_id, JobContext};
use epistemic_core::domain::{
    job_kind, ReviewStatus, SourceLayer, Job,
};
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

    // Prefer TEI text; fall back to title+abstract
    let paper_text = if let Some(ref tei_rel) = version.tei_path {
        let p = ctx.tei_dir.join(tei_rel);
        if p.exists() {
            let tei = tokio::fs::read_to_string(&p).await?;
            strip_xml_rough(&tei)
        } else {
            format!("{}\n\n{}", version.title, version.abstract_text)
        }
    } else {
        format!("{}\n\n{}", version.title, version.abstract_text)
    };

    // Truncate to keep cost bounded
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

    // Record extraction
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

    // Persist claims / methods with evidence
    let work_id = version.work_id;
    if let Some(claims) = value.get("claims").and_then(|c| c.as_array()) {
        for c in claims {
            let text = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = c.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = c.get("page").and_then(|p| p.as_i64()).unwrap_or(1) as i32;
            if text.is_empty() || source_text.is_empty() {
                continue; // principle: no evidence → discard
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
            .bind(format!("{model}/{PROMPT_VERSION}"))
            .fetch_one(&ctx.pool)
            .await?;

            sqlx::query(
                r#"
                INSERT INTO evidence_spans (claim_id, version_id, page, text, extraction_field)
                VALUES ($1, $2, $3, $4, 'claim')
                "#,
            )
            .bind(claim_id)
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .execute(&ctx.pool)
            .await?;
        }
    }

    if let Some(methods) = value.get("methods").and_then(|m| m.as_array()) {
        for m in methods {
            let name = m.get("name").and_then(|t| t.as_str()).unwrap_or("");
            let desc = m.get("description").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = m.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            if name.is_empty() || source_text.is_empty() {
                continue;
            }
            sqlx::query(
                r#"
                INSERT INTO methods (work_id, name, description, source, review_status, model_version)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(work_id)
            .bind(name)
            .bind(desc)
            .bind(SourceLayer::AiCandidate)
            .bind(ReviewStatus::Unreviewed)
            .bind(format!("{model}/{PROMPT_VERSION}"))
            .execute(&ctx.pool)
            .await?;
        }
    }

    // Chain citation context classification (M3 stub still runs as no-op until implemented)
    let payload = serde_json::json!({
        "version_id": vid,
        "work_id": work_id,
    });
    jobs::enqueue(&ctx.pool, job_kind::CLASSIFY_CITATION_CONTEXTS, payload.clone()).await?;
    jobs::enqueue(&ctx.pool, job_kind::EMBED, payload).await?;

    tracing::info!(%vid, cost_usd = cost, "DNA extraction done");
    Ok(())
}

/// Very rough TEI → text: strip tags, collapse whitespace.
fn strip_xml_rough(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
