use super::{version_id, JobContext};
use crate::arxiv_html;
use crate::metadata::RefItem;
use crate::pdf_render;
use epistemic_core::domain::{job_kind, Job, ReviewStatus, SourceLayer, ASPECTS};
use epistemic_core::repo::{jobs, works};
use epistemic_llm::{estimate_cost_usd, Usage};
use uuid::Uuid;

const PROMPT_VERSION: &str = "dna_aspects_v1";
const MAX_PAPER_CHARS: usize = 120_000;

type ExtractionOutput = (serde_json::Value, Usage, String, f64, String);

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;
    let version_id = version_id(job)?;
    let version = works::get_version(&ctx.pool, version_id).await?;

    let (value, _usage, model, cost, status) = if let Some(saved) = checkpoint(ctx, job.id).await? {
        saved
    } else {
        let (value, usage, model) = extract_from_source(ctx, llm, &version).await?;
        let cost = estimate_cost_usd(&model, &usage);
        sqlx::query(
            r#"
            INSERT INTO extractions (
                version_id, model, prompt_version, raw, status, usage, cost_usd, job_id
            )
            VALUES ($1, $2, $3, $4, 'ready', $5, $6, $7)
            ON CONFLICT (job_id) WHERE job_id IS NOT NULL DO NOTHING
            "#,
        )
        .bind(version_id)
        .bind(&model)
        .bind(PROMPT_VERSION)
        .bind(&value)
        .bind(serde_json::to_value(&usage)?)
        .bind(cost)
        .bind(job.id)
        .execute(&ctx.pool)
        .await?;
        checkpoint(ctx, job.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to persist extraction checkpoint"))?
    };

    // mark_done may have failed after a successful commit; avoid rebuilding or
    // re-enqueueing downstream work on that retry.
    if status == "done" {
        return Ok(());
    }

    let mut tx = ctx.pool.begin().await?;
    lock_work(&mut tx, version.work_id).await?;
    replace_generated_data(&mut tx, version_id, version.work_id, &model, &value).await?;

    let payload = serde_json::json!({
        "version_id": version_id,
        "work_id": version.work_id,
    });
    for kind in [
        job_kind::EMBED,
        job_kind::UPDATE_NEIGHBORS_CITATION,
        job_kind::PROPOSE_PAIRS,
    ] {
        jobs::enqueue_unique_tx(
            &mut tx,
            kind,
            payload.clone(),
            &format!("pipeline:{kind}:{version_id}"),
        )
        .await?;
    }
    sqlx::query("UPDATE extractions SET status = 'done' WHERE job_id = $1")
        .bind(job.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    tracing::info!(%version_id, cost_usd = cost, "DNA extraction materialized");
    Ok(())
}

async fn checkpoint(ctx: &JobContext, job_id: Uuid) -> anyhow::Result<Option<ExtractionOutput>> {
    let row = sqlx::query_as::<
        _,
        (
            Option<serde_json::Value>,
            Option<serde_json::Value>,
            String,
            Option<f64>,
            String,
        ),
    >("SELECT raw, usage, model, cost_usd, status FROM extractions WHERE job_id = $1")
    .bind(job_id)
    .fetch_optional(&ctx.pool)
    .await?;
    let Some((Some(raw), Some(usage), model, cost, status)) = row else {
        return Ok(None);
    };
    Ok(Some((
        raw,
        serde_json::from_value(usage)?,
        model,
        cost.unwrap_or_default(),
        status,
    )))
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

async fn replace_generated_data(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
    work_id: Uuid,
    model: &str,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    // Claim evidence is removed by the claim cascade. Field/method evidence has
    // no owning entity FK, so delete it explicitly before replacement.
    sqlx::query(
        r#"
        DELETE FROM evidence_spans
        WHERE version_id = $1
          AND relation_id IS NULL
          AND claim_id IS NULL
          AND extraction_field IS NOT NULL
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
    sqlx::query("DELETE FROM paper_aspects WHERE work_id = $1")
        .bind(work_id)
        .execute(&mut **tx)
        .await?;

    let model_version = format!("{model}/{PROMPT_VERSION}");
    for definition in ASPECTS {
        let Some(object) = value.get(definition.key) else {
            continue;
        };
        let summary = object
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        let bullets = object
            .get("bullets")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let source_text = object
            .get("source_text")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let page = non_negative_page(object);

        sqlx::query(
            r#"
            INSERT INTO paper_aspects (
                work_id, aspect, summary, bullets, source_text, page, model, prompt_version
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(work_id)
        .bind(definition.key)
        .bind(summary)
        .bind(&bullets)
        .bind(source_text)
        .bind(page)
        .bind(model)
        .bind(PROMPT_VERSION)
        .execute(&mut **tx)
        .await?;

        if !source_text.is_empty() {
            insert_field_evidence(tx, version_id, definition.key, object).await?;
        }
        match definition.key {
            "methods" => {
                materialize_methods(tx, work_id, version_id, object, &model_version).await?;
            }
            "findings" => {
                materialize_claims(tx, work_id, version_id, object, &model_version).await?;
            }
            _ => {}
        }
    }

    if let Some(references) = value.get("references").and_then(|value| value.as_array()) {
        let items = parse_references(references);
        replace_citations(tx, work_id, &items).await?;
    }
    Ok(())
}

fn non_negative_page(object: &serde_json::Value) -> i32 {
    object
        .get("page")
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .max(0) as i32
}

async fn materialize_methods(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_id: Uuid,
    version_id: Uuid,
    object: &serde_json::Value,
    model_version: &str,
) -> anyhow::Result<()> {
    let bullets = object
        .get("bullets")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = object
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    let source_text = object
        .get("source_text")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let page = non_negative_page(object);
    let names: Vec<String> = if bullets.is_empty() {
        (!summary.is_empty())
            .then(|| summary.chars().take(120).collect())
            .into_iter()
            .collect()
    } else {
        bullets
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    };

    for name in names {
        let description = if name == summary { "" } else { summary };
        sqlx::query(
            r#"
            INSERT INTO methods (work_id, name, description, source, review_status, model_version)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(work_id)
        .bind(&name)
        .bind(description)
        .bind(SourceLayer::AiCandidate)
        .bind(ReviewStatus::Unreviewed)
        .bind(model_version)
        .execute(&mut **tx)
        .await?;
        if !source_text.is_empty() {
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (version_id, page, text, extraction_field)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(version_id)
            .bind(page)
            .bind(source_text)
            .bind(format!("method:{name}"))
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn materialize_claims(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_id: Uuid,
    version_id: Uuid,
    object: &serde_json::Value,
    model_version: &str,
) -> anyhow::Result<()> {
    let bullets = object
        .get("bullets")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let summary = object
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    let source_text = object
        .get("source_text")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let page = non_negative_page(object);
    let texts: Vec<String> = if bullets.is_empty() {
        (!summary.is_empty())
            .then(|| summary.to_owned())
            .into_iter()
            .collect()
    } else {
        bullets
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    };

    for text in texts {
        let claim_id: Uuid = sqlx::query_scalar(
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
        .bind(model_version)
        .fetch_one(&mut **tx)
        .await?;
        if !source_text.is_empty() {
            sqlx::query(
                r#"
                INSERT INTO evidence_spans (claim_id, version_id, page, text, extraction_field)
                VALUES ($1, $2, $3, $4, 'claim')
                "#,
            )
            .bind(claim_id)
            .bind(version_id)
            .bind(page)
            .bind(source_text)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

fn parse_references(values: &[serde_json::Value]) -> Vec<RefItem> {
    values
        .iter()
        .filter_map(|reference| {
            let title = reference
                .get("title")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let arxiv_id = reference
                .get("arxiv_id")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .map(|value| value.trim_start_matches("arXiv:"))
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let doi = reference
                .get("doi")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
            let year = reference
                .get("year")
                .and_then(|value| value.as_i64())
                .map(|value| value as i32);
            (title.is_some() || arxiv_id.is_some() || doi.is_some()).then_some(RefItem {
                title,
                arxiv_id,
                doi,
                year,
            })
        })
        .collect()
}

async fn replace_citations(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_id: Uuid,
    references: &[RefItem],
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM citations WHERE citing_work_id = $1")
        .bind(work_id)
        .execute(&mut **tx)
        .await?;
    for reference in references {
        let cited_work_id: Option<Uuid> = if let Some(arxiv_id) = &reference.arxiv_id {
            sqlx::query_scalar("SELECT work_id FROM versions WHERE arxiv_id = $1")
                .bind(arxiv_id)
                .fetch_optional(&mut **tx)
                .await?
        } else if let Some(doi) = &reference.doi {
            sqlx::query_scalar("SELECT work_id FROM versions WHERE doi = $1")
                .bind(doi)
                .fetch_optional(&mut **tx)
                .await?
        } else {
            None
        };
        let external = cited_work_id.is_none().then(|| {
            serde_json::json!({
                "title": reference.title,
                "arxiv_id": reference.arxiv_id,
                "doi": reference.doi,
                "year": reference.year,
            })
        });
        sqlx::query(
            "INSERT INTO citations (citing_work_id, cited_work_id, cited_external) VALUES ($1, $2, $3)",
        )
        .bind(work_id)
        .bind(cited_work_id)
        .bind(external)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn insert_field_evidence(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
    field: &str,
    object: &serde_json::Value,
) -> anyhow::Result<()> {
    let source_text = object
        .get("source_text")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if source_text.is_empty() {
        return Ok(());
    }
    let text = object
        .get("summary")
        .or_else(|| object.get("text"))
        .or_else(|| object.get("name"))
        .and_then(|value| value.as_str())
        .unwrap_or(source_text);
    sqlx::query(
        r#"
        INSERT INTO evidence_spans (version_id, page, text, extraction_field, bbox)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(version_id)
    .bind(non_negative_page(object))
    .bind(text)
    .bind(field)
    .bind(object.get("bbox").cloned())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn extract_from_source(
    ctx: &JobContext,
    llm: &epistemic_llm::LlmClient,
    version: &epistemic_core::domain::Version,
) -> anyhow::Result<(serde_json::Value, Usage, String)> {
    let system = include_str!("../../../llm/prompts/dna_aspects_v1.md");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../llm/prompts/dna_aspects_schema_v1.json"
    ))?;
    let aspect_keys = ASPECTS
        .iter()
        .map(|aspect| aspect.key)
        .collect::<Vec<_>>()
        .join(", ");

    if let Some(relative_path) = &version.pdf_path {
        let pdf_path = ctx.pdf_dir.join(relative_path);
        if pdf_path.exists() {
            let images = pdf_render::pdf_to_png_data_urls(&pdf_path).await?;
            let prompt = format!(
                "Paper title: {}\nThe attached images are consecutive PDF pages in order. \
                 Extract the 8 fixed aspects ({aspect_keys}) and the bibliography. \
                 Use the one-based image position as page. Respond with matching JSON only.",
                version.title
            );
            tracing::info!(
                version_id = %version.id,
                pages = images.len(),
                model = llm.model(),
                "extracting DNA from PDF page images"
            );
            return Ok(llm
                .complete_json_vision(system, &prompt, &images, schema, 16_000)
                .await?);
        }
        anyhow::bail!("configured PDF is missing: {}", pdf_path.display());
    }

    let mut full = None;
    if let Some(relative_path) = &version.tei_path {
        if relative_path.ends_with(".html") {
            let path = ctx.pdf_dir.join(relative_path);
            if path.exists() {
                let html = tokio::fs::read_to_string(&path).await?;
                let text = arxiv_html::html_to_text(&html);
                if text.chars().count() > 200 {
                    full = Some((text, format!("file:{relative_path}")));
                }
            }
        }
    }
    if full.is_none() {
        if let Some(arxiv_id) = &version.arxiv_id {
            match arxiv_html::fetch_arxiv_html(&ctx.http, arxiv_id).await {
                Ok(document) => {
                    let relative_path = format!("{}/{arxiv_id}.html", version.id);
                    let destination = ctx.pdf_dir.join(&relative_path);
                    if let Some(parent) = destination.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&destination, document.html.as_bytes()).await?;
                    works::update_version_paths(&ctx.pool, version.id, None, Some(&relative_path))
                        .await?;
                    full = Some((document.full_text, document.url));
                }
                Err(error) => tracing::warn!(%arxiv_id, %error, "HTML fetch failed"),
            }
        }
    }

    let (paper_text, source) = full.unwrap_or_else(|| {
        tracing::warn!(version_id = %version.id, "no PDF or HTML; using title and abstract");
        (
            format!("{}\n\n{}", version.title, version.abstract_text),
            "title_abstract".to_owned(),
        )
    });
    let paper_text: String = paper_text.chars().take(MAX_PAPER_CHARS).collect();
    let prompt = format!(
        "Paper title: {}\nSource: {source}.\n\
         Extract the 8 fixed aspects ({aspect_keys}) and full bibliography.\n\
         Page may be 0 when unknown. Respond with matching JSON only.\n\n\
         --- BEGIN PAPER ---\n{paper_text}\n--- END PAPER ---",
        version.title
    );
    Ok(llm.complete_json(system, &prompt, schema, 16_000).await?)
}

#[cfg(test)]
mod tests {
    use epistemic_core::domain::{AspectDef, ASPECTS};

    #[test]
    fn schema_parses_and_has_all_aspects() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../llm/prompts/dna_aspects_schema_v1.json"
        ))
        .unwrap();
        let properties = schema.get("properties").unwrap();
        for definition in ASPECTS {
            assert!(properties.get(definition.key).is_some());
        }
        assert!(properties.get("references").is_some());
    }

    #[test]
    fn aspect_defs_are_stable() {
        assert_eq!(ASPECTS.len(), 8);
        assert!(AspectDef::by_key("methods").is_some());
    }
}
