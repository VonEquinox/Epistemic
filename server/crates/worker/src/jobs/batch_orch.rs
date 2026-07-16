//! Batch API orchestration for bulk DNA extraction.

use super::{JobContext, JobOutcome};
use epistemic_core::domain::{job_kind, Job, ReviewStatus, SourceLayer};
use epistemic_core::repo::{jobs, works};
use epistemic_llm::{estimate_cost_usd, BatchRequestItem, BatchResultLine};
use std::collections::HashMap;
use uuid::Uuid;

const PROMPT_VERSION: &str = "dna_v1";
const POLL_DELAY_SECS: i64 = 30;

pub async fn run(ctx: &JobContext, job: &Job, worker_id: &str) -> anyhow::Result<JobOutcome> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;
    let version_ids = version_ids(job);
    if version_ids.is_empty() {
        anyhow::bail!("batch_orch: version_ids empty");
    }

    if let Some(batch_id) = job
        .payload
        .get("batch_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
    {
        return poll_and_apply(ctx, llm, job, worker_id, batch_id, &version_ids).await;
    }

    // Persist an ambiguity guard before making the non-transactional paid API call.
    // If the response is lost or the following DB update fails, retrying this job
    // will not submit a second batch automatically.
    if job
        .payload
        .get("submission_started")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        anyhow::bail!(
            "batch submission outcome is ambiguous; refusing automatic duplicate submission"
        );
    }
    let system = include_str!("../../../llm/prompts/dna_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/dna_schema_v1.json"))?;
    let mut items = Vec::with_capacity(version_ids.len());
    for vid in &version_ids {
        let version = works::get_version(&ctx.pool, *vid).await?;
        let params = if let Some(rel) = &version.pdf_path {
            let path = ctx.pdf_dir.join(rel);
            let images = crate::pdf_render::pdf_to_png_data_urls(&path).await?;
            llm.json_request_vision(
                system,
                "Extract Paper DNA from all supplied PDF page images.",
                &images,
                schema.clone(),
                16_000,
            )
        } else {
            let paper_text: String = paper_text(ctx, &version)
                .await?
                .chars()
                .take(80_000)
                .collect();
            let user = format!(
                "Extract Paper DNA from the following paper text.\n\n---\n{paper_text}\n---"
            );
            llm.json_request(system, &user, schema.clone(), 8000)
        };
        items.push(BatchRequestItem {
            custom_id: vid.to_string(),
            params,
        });
    }

    let mut started_payload = job.payload.clone();
    started_payload["submission_started"] = serde_json::Value::Bool(true);
    update_owned_payload(ctx, job.id, worker_id, &started_payload).await?;

    tracing::info!(n = items.len(), "submitting DNA batch");
    let handle = llm.create_batch(items).await?;
    let payload = serde_json::json!({
        "version_ids": version_ids.iter().map(Uuid::to_string).collect::<Vec<_>>(),
        "batch_id": handle.id,
        "kind": "extract_dna",
        "submission_started": true,
    });
    jobs::reschedule(&ctx.pool, job.id, worker_id, payload, POLL_DELAY_SECS).await?;
    Ok(JobOutcome::Rescheduled)
}

fn version_ids(job: &Job) -> Vec<Uuid> {
    job.payload
        .get("version_ids")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str())
                .filter_map(|value| Uuid::parse_str(value).ok())
                .collect()
        })
        .unwrap_or_default()
}

async fn update_owned_payload(
    ctx: &JobContext,
    job_id: Uuid,
    worker_id: &str,
    payload: &serde_json::Value,
) -> anyhow::Result<()> {
    let result = sqlx::query(
        "UPDATE jobs SET payload = $3 WHERE id = $1 AND status = 'running' AND locked_by = $2",
    )
    .bind(job_id)
    .bind(worker_id)
    .bind(payload)
    .execute(&ctx.pool)
    .await?;
    if result.rows_affected() == 0 {
        anyhow::bail!("batch job lease is no longer owned by {worker_id}");
    }
    Ok(())
}

async fn poll_and_apply(
    ctx: &JobContext,
    llm: &epistemic_llm::LlmClient,
    job: &Job,
    worker_id: &str,
    batch_id: &str,
    version_ids: &[Uuid],
) -> anyhow::Result<JobOutcome> {
    let handle = llm.get_batch(batch_id).await?;
    if !handle.is_ended() {
        jobs::reschedule(
            &ctx.pool,
            job.id,
            worker_id,
            job.payload.clone(),
            POLL_DELAY_SECS,
        )
        .await?;
        return Ok(JobOutcome::Rescheduled);
    }
    if !handle.is_success_ended() {
        anyhow::bail!("batch {batch_id} ended with status {}", handle.status);
    }

    let result_map: HashMap<String, BatchResultLine> = llm
        .batch_results(batch_id)
        .await?
        .into_iter()
        .map(|line| (line.custom_id.clone(), line))
        .collect();
    let mut errors = Vec::new();

    for version_id in version_ids {
        let custom_id = version_id.to_string();
        let Some(line) = result_map.get(&custom_id) else {
            errors.push(format!("{custom_id}: missing batch output"));
            continue;
        };
        if let Some(error) = &line.result.error {
            errors.push(format!("{custom_id}: provider error {error}"));
            continue;
        }
        let Some(message) = &line.result.message else {
            errors.push(format!("{custom_id}: missing response message"));
            continue;
        };
        let Some(text) = message.text() else {
            errors.push(format!("{custom_id}: response has no text"));
            continue;
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                errors.push(format!("{custom_id}: invalid JSON: {error}"));
                continue;
            }
        };

        let mut tx = ctx.pool.begin().await?;
        let claimed: Option<i32> = sqlx::query_scalar(
            r#"
            INSERT INTO batch_applied_items (batch_id, custom_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            RETURNING 1
            "#,
        )
        .bind(batch_id)
        .bind(&custom_id)
        .fetch_optional(&mut *tx)
        .await?;
        if claimed.is_none() {
            tx.rollback().await?;
            continue;
        }

        let version = works::get_version(&ctx.pool, *version_id).await?;
        lock_work(&mut tx, version.work_id).await?;
        let cost = estimate_cost_usd(&message.model, &message.usage) * 0.5;
        sqlx::query(
            r#"
            INSERT INTO extractions (version_id, model, prompt_version, raw, status, usage, cost_usd)
            VALUES ($1, $2, $3, $4, 'done', $5, $6)
            "#,
        )
        .bind(version_id)
        .bind(&message.model)
        .bind(PROMPT_VERSION)
        .bind(&value)
        .bind(serde_json::to_value(&message.usage)?)
        .bind(cost)
        .execute(&mut *tx)
        .await?;

        replace_dna_fields(
            &mut tx,
            *version_id,
            version.work_id,
            &message.model,
            &value,
        )
        .await?;
        let payload = serde_json::json!({
            "version_id": version_id,
            "work_id": version.work_id,
        });
        jobs::enqueue_unique_tx(
            &mut tx,
            job_kind::CLASSIFY_CITATION_CONTEXTS,
            payload.clone(),
            &format!(
                "pipeline:{}:{}",
                job_kind::CLASSIFY_CITATION_CONTEXTS,
                version_id
            ),
        )
        .await?;
        jobs::enqueue_unique_tx(
            &mut tx,
            job_kind::EMBED,
            payload,
            &format!("pipeline:{}:{}", job_kind::EMBED, version_id),
        )
        .await?;
        tx.commit().await?;
    }

    if !errors.is_empty() {
        anyhow::bail!("batch {batch_id} had item failures: {}", errors.join("; "));
    }
    Ok(JobOutcome::Done)
}

async fn lock_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(work_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn paper_text(
    ctx: &JobContext,
    version: &epistemic_core::domain::Version,
) -> anyhow::Result<String> {
    if let Some(ref rel) = version.tei_path {
        let path = if rel.ends_with(".html") {
            ctx.pdf_dir.join(rel)
        } else {
            ctx.tei_dir.join(rel)
        };
        if path.exists() {
            let source = tokio::fs::read_to_string(path).await?;
            return Ok(strip_xml_rough(&source));
        }
    }
    Ok(format!("{}\n\n{}", version.title, version.abstract_text))
}

fn strip_xml_rough(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut in_tag = false;
    for character in xml.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn replace_dna_fields(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
    work_id: Uuid,
    model: &str,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        DELETE FROM evidence_spans
        WHERE version_id = $1 AND claim_id IS NULL
          AND extraction_field LIKE 'method:%'
        "#,
    )
    .bind(version_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM claims WHERE work_id = $1 AND source = 'ai_candidate'")
        .bind(work_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM methods WHERE work_id = $1 AND source = 'ai_candidate'")
        .bind(work_id)
        .execute(&mut **tx)
        .await?;

    let model_version = format!("{model}/{PROMPT_VERSION}");
    if let Some(claims) = value.get("claims").and_then(|value| value.as_array()) {
        for claim in claims {
            let text = claim
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let source_text = claim
                .get("source_text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let page = claim
                .get("page")
                .and_then(|value| value.as_i64())
                .unwrap_or(0) as i32;
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
            .bind(&model_version)
            .fetch_one(&mut **tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (claim_id, version_id, page, text, extraction_field, bbox)
                VALUES ($1, $2, $3, $4, 'claim', $5)
                "#,
            )
            .bind(claim_id)
            .bind(version_id)
            .bind(page)
            .bind(source_text)
            .bind(claim.get("bbox").cloned())
            .execute(&mut **tx)
            .await?;
        }
    }
    if let Some(methods) = value.get("methods").and_then(|value| value.as_array()) {
        for method in methods {
            let name = method
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let description = method
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let source_text = method
                .get("source_text")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let page = method
                .get("page")
                .and_then(|value| value.as_i64())
                .unwrap_or(0) as i32;
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
            .bind(description)
            .bind(SourceLayer::AiCandidate)
            .bind(ReviewStatus::Unreviewed)
            .bind(&model_version)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (version_id, page, text, extraction_field, bbox)
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(version_id)
            .bind(page)
            .bind(source_text)
            .bind(format!("method:{name}"))
            .bind(method.get("bbox").cloned())
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}
