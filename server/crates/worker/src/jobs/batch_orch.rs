//! Batch API orchestration for bulk DNA extraction (DEV.md §8.3).
//!
//! Payload:
//! ```json
//! { "kind": "extract_dna", "version_ids": ["..."], "batch_id": null | "msgbatch_..." }
//! ```
//! - If `batch_id` is null: build requests, submit batch, re-enqueue self with batch_id + run_after.
//! - If set: poll; when ended, parse results and write extractions / claims like extract.rs.

use super::JobContext;
use epistemic_core::domain::{job_kind, ReviewStatus, SourceLayer, Job};
use epistemic_core::repo::{jobs, works};
use epistemic_llm::{estimate_cost_usd, BatchRequestItem};
use uuid::Uuid;

const PROMPT_VERSION: &str = "dna_v1";

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;

    let version_ids: Vec<Uuid> = job
        .payload
        .get("version_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect()
        })
        .unwrap_or_default();

    if version_ids.is_empty() {
        anyhow::bail!("batch_orch: version_ids empty");
    }

    if let Some(batch_id) = job
        .payload
        .get("batch_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return poll_and_apply(ctx, llm, batch_id, &version_ids).await;
    }

    // Submit new batch
    let system = include_str!("../../../llm/prompts/dna_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/dna_schema_v1.json"))?;

    let mut items = Vec::new();
    for vid in &version_ids {
        let version = works::get_version(&ctx.pool, *vid).await?;
        let paper_text = paper_text(ctx, &version).await?;
        let paper_text: String = paper_text.chars().take(80_000).collect();
        let user = format!(
            "Extract Paper DNA from the following paper text.\n\n---\n{paper_text}\n---"
        );
        let params = llm.json_request(system, &user, schema.clone(), 8000);
        items.push(BatchRequestItem {
            custom_id: vid.to_string(),
            params,
        });
    }

    tracing::info!(n = items.len(), "submitting DNA batch");
    let handle = llm.create_batch(items).await?;
    tracing::info!(batch_id = %handle.id, status = %handle.processing_status, "batch submitted");

    // Re-enqueue poll job
    let payload = serde_json::json!({
        "version_ids": version_ids.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        "batch_id": handle.id,
        "kind": "extract_dna",
    });
    // Use delayed run via SQL run_after — enqueue then update
    let job_row = jobs::enqueue(&ctx.pool, job_kind::BATCH_ORCH, payload).await?;
    sqlx::query(
        r#"UPDATE jobs SET run_after = now() + interval '30 seconds' WHERE id = $1"#,
    )
    .bind(job_row.id)
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

async fn poll_and_apply(
    ctx: &JobContext,
    llm: &epistemic_llm::ClaudeClient,
    batch_id: &str,
    version_ids: &[Uuid],
) -> anyhow::Result<()> {
    let handle = llm.get_batch(batch_id).await?;
    if handle.processing_status != "ended" {
        tracing::info!(
            batch_id,
            status = %handle.processing_status,
            "batch still running; re-queue"
        );
        let payload = serde_json::json!({
            "version_ids": version_ids.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            "batch_id": batch_id,
            "kind": "extract_dna",
        });
        let job_row = jobs::enqueue(&ctx.pool, job_kind::BATCH_ORCH, payload).await?;
        sqlx::query(
            r#"UPDATE jobs SET run_after = now() + interval '30 seconds' WHERE id = $1"#,
        )
        .bind(job_row.id)
        .execute(&ctx.pool)
        .await?;
        return Ok(());
    }

    let results = llm.batch_results(batch_id).await?;
    tracing::info!(batch_id, n = results.len(), "applying batch results");

    for line in results {
        let Ok(vid) = Uuid::parse_str(&line.custom_id) else {
            continue;
        };
        let Some(msg) = line.result.message else {
            tracing::warn!(custom_id = %line.custom_id, "batch item has no message");
            continue;
        };
        let Some(text) = msg.text() else {
            continue;
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, %vid, "batch DNA json parse failed");
                continue;
            }
        };
        let cost = estimate_cost_usd(&msg.model, &msg.usage);
        // Half cost accounting for batch discount
        let cost = cost * 0.5;

        sqlx::query(
            r#"
            INSERT INTO extractions (version_id, model, prompt_version, raw, status, usage, cost_usd)
            VALUES ($1, $2, $3, $4, 'done', $5, $6)
            "#,
        )
        .bind(vid)
        .bind(&msg.model)
        .bind(PROMPT_VERSION)
        .bind(&value)
        .bind(serde_json::to_value(&msg.usage)?)
        .bind(cost)
        .execute(&ctx.pool)
        .await?;

        // Reuse extract path logic lightly: enqueue classify
        let version = works::get_version(&ctx.pool, vid).await?;
        apply_dna_fields(&ctx.pool, vid, version.work_id, &msg.model, &value).await?;
        jobs::enqueue(
            &ctx.pool,
            job_kind::CLASSIFY_CITATION_CONTEXTS,
            serde_json::json!({ "version_id": vid, "work_id": version.work_id }),
        )
        .await?;
        jobs::enqueue(
            &ctx.pool,
            job_kind::EMBED,
            serde_json::json!({ "version_id": vid, "work_id": version.work_id }),
        )
        .await?;
    }
    Ok(())
}

async fn paper_text(
    ctx: &JobContext,
    version: &epistemic_core::domain::Version,
) -> anyhow::Result<String> {
    if let Some(ref tei_rel) = version.tei_path {
        let p = ctx.tei_dir.join(tei_rel);
        if p.exists() {
            let tei = tokio::fs::read_to_string(&p).await?;
            return Ok(strip_xml_rough(&tei));
        }
    }
    Ok(format!("{}\n\n{}", version.title, version.abstract_text))
}

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

async fn apply_dna_fields(
    pool: &sqlx::PgPool,
    vid: Uuid,
    work_id: Uuid,
    model: &str,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    let model_ver = format!("{model}/{PROMPT_VERSION}");
    if let Some(claims) = value.get("claims").and_then(|c| c.as_array()) {
        for c in claims {
            let text = c.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = c.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = c.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            if text.is_empty() || source_text.is_empty() || page < 1 {
                continue;
            }
            let claim_id: Uuid = sqlx::query_scalar(
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
            .fetch_one(pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (claim_id, version_id, page, text, extraction_field, bbox)
                VALUES ($1, $2, $3, $4, 'claim', $5)
                "#,
            )
            .bind(claim_id)
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .bind(c.get("bbox").cloned())
            .execute(pool)
            .await?;
        }
    }
    if let Some(methods) = value.get("methods").and_then(|m| m.as_array()) {
        for m in methods {
            let name = m.get("name").and_then(|t| t.as_str()).unwrap_or("");
            let desc = m.get("description").and_then(|t| t.as_str()).unwrap_or("");
            let source_text = m.get("source_text").and_then(|t| t.as_str()).unwrap_or("");
            let page = m.get("page").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            if name.is_empty() || source_text.is_empty() || page < 1 {
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
            .bind(&model_ver)
            .execute(pool)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (version_id, page, text, extraction_field, bbox)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(vid)
            .bind(page)
            .bind(source_text)
            .bind(format!("method:{name}"))
            .bind(m.get("bbox").cloned())
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}
