//! Pairwise relation proposal — PRD §6.3 path 2 (limited).
//!
//! Without embeddings yet: recall candidates from citation neighbors +
//! method-lineage neighbors + shared-project co-members, take top-K,
//! then LLM pairwise compare. Write ai_candidate relations for conf ≥ 0.5.

use super::{version_id, work_id, JobContext};
use epistemic_core::domain::{
    EntityKind, Job, MemberRole, RelationType, ReviewStatus, SourceLayer,
};
use epistemic_core::repo::relations::{NewEvidence, NewRelation, NewRelationMember};
use epistemic_core::repo::{relations, works};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const PROMPT_VERSION: &str = "pair_v1";
const MIN_CONFIDENCE: f64 = 0.5;
const K: usize = 10;

#[derive(Debug, Clone)]
struct PaperBrief {
    work_id: Uuid,
    version_id: Uuid,
    title: String,
    abstract_text: String,
    methods: Vec<String>,
    claims: Vec<String>,
}

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let seed_work = work_id(job).unwrap_or(version.work_id);

    let seed = load_brief(ctx, seed_work).await?;
    let Some(seed) = seed else {
        tracing::warn!(%seed_work, "seed work missing primary version");
        return Ok(());
    };

    let candidates = recall_candidates(ctx, seed_work).await?;
    if candidates.is_empty() {
        tracing::info!(%seed_work, "no pair candidates recalled");
        return Ok(());
    }

    let system = include_str!("../../../llm/prompts/pair_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/pair_schema_v1.json"))?;
    let model_ver = format!("{}/{PROMPT_VERSION}", llm.model());

    let mut created = 0u32;
    for other_id in candidates.into_iter().take(K) {
        let Some(other) = load_brief(ctx, other_id).await? else {
            continue;
        };
        let user = build_pair_prompt(&seed, &other);
        let (value, usage, model) = match llm
            .complete_json(system, &user, schema.clone(), 2500)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, other = %other_id, "pair LLM failed");
                continue;
            }
        };
        let cost = epistemic_llm::estimate_cost_usd(&model, &usage);
        tracing::debug!(
            other = %other_id,
            cost_usd = cost,
            "pair comparison done"
        );

        let items = value
            .get("relations")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for item in items {
            let type_s = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let conf = item
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let Some(rtype) = RelationType::from_llm(type_s) else {
                continue;
            };
            if conf < MIN_CONFIDENCE {
                continue;
            }

            let src_tag = item
                .get("source_paper")
                .and_then(|v| v.as_str())
                .unwrap_or("A");
            let tgt_tag = item
                .get("target_paper")
                .and_then(|v| v.as_str())
                .unwrap_or("B");
            let (src, tgt) = match (src_tag, tgt_tag) {
                ("A", "B") => (&seed, &other),
                ("B", "A") => (&other, &seed),
                _ => continue,
            };
            if src.work_id == tgt.work_id {
                continue;
            }
            if relation_exists(&ctx.pool, rtype, src.work_id, tgt.work_id).await? {
                continue;
            }

            let explanation = item
                .get("explanation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let evidence_quote = item
                .get("evidence_quote")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if evidence_quote.trim().is_empty() {
                continue;
            }
            let evidence_from = item
                .get("evidence_from")
                .and_then(|v| v.as_str())
                .unwrap_or(src_tag);
            let evidence_version = if evidence_from == "A" {
                seed.version_id
            } else {
                other.version_id
            };
            // Pairwise has no page from abstract — store page 0 (UI falls back to text search)
            let page = 0i32;

            let aspect = item
                .get("aspect")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            relations::create_relation(
                &ctx.pool,
                NewRelation {
                    relation_type: rtype,
                    aspect,
                    scope: None,
                    explanation,
                    confidence: Some(conf),
                    source: SourceLayer::AiCandidate,
                    review_status: ReviewStatus::Unreviewed,
                    created_by_user: None,
                    model_version: Some(model_ver.clone()),
                    members: vec![
                        NewRelationMember {
                            entity_kind: EntityKind::Work,
                            entity_id: src.work_id,
                            role: MemberRole::Source,
                            anchor_work_id: Some(src.work_id),
                            position: 0,
                        },
                        NewRelationMember {
                            entity_kind: EntityKind::Work,
                            entity_id: tgt.work_id,
                            role: MemberRole::Target,
                            anchor_work_id: Some(tgt.work_id),
                            position: 1,
                        },
                    ],
                    evidence: vec![NewEvidence {
                        version_id: evidence_version,
                        page,
                        text: evidence_quote,
                        bbox: None,
                    }],
                },
            )
            .await?;
            created += 1;
        }
    }

    tracing::info!(%seed_work, created, "propose_pairs done");
    Ok(())
}

async fn load_brief(ctx: &JobContext, work_id: Uuid) -> anyhow::Result<Option<PaperBrief>> {
    let work = match works::get_work(&ctx.pool, work_id).await {
        Ok(w) => w,
        Err(_) => return Ok(None),
    };
    let Some(vid) = work.primary_version_id else {
        return Ok(None);
    };
    let version = works::get_version(&ctx.pool, vid).await?;
    let methods = works::list_methods(&ctx.pool, work_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            if m.description.is_empty() {
                m.name
            } else {
                format!("{}: {}", m.name, m.description)
            }
        })
        .collect();
    let claims = works::list_claims(&ctx.pool, work_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.text)
        .collect();
    Ok(Some(PaperBrief {
        work_id,
        version_id: vid,
        title: version.title,
        abstract_text: version.abstract_text,
        methods,
        claims,
    }))
}

/// Recall K candidates without embeddings: citation neighbors + method lineage
/// + co-project works, ranked by a simple heuristic score.
async fn recall_candidates(ctx: &JobContext, seed: Uuid) -> anyhow::Result<Vec<Uuid>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        other_id: Uuid,
        score: f64,
    }

    // 1) neighbors table (citation_coupling + method_lineage + topic)
    let from_neighbors: Vec<Row> = sqlx::query_as(
        r#"
        SELECT neighbor_work_id AS other_id, MAX(score) AS score
        FROM neighbors
        WHERE work_id = $1
          AND dimension IN ('citation_coupling', 'method_lineage', 'topic')
        GROUP BY neighbor_work_id
        ORDER BY score DESC
        LIMIT 32
        "#,
    )
    .bind(seed)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    // 2) direct citation edges (in either direction) that resolved to a work
    let from_cites: Vec<Row> = sqlx::query_as(
        r#"
        SELECT cited_work_id AS other_id, 0.55::float AS score
        FROM citations
        WHERE citing_work_id = $1 AND cited_work_id IS NOT NULL
        UNION
        SELECT citing_work_id AS other_id, 0.50::float AS score
        FROM citations
        WHERE cited_work_id = $1
        "#,
    )
    .bind(seed)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    // 3) co-project works (weak signal)
    let from_project: Vec<Row> = sqlx::query_as(
        r#"
        SELECT wp2.work_id AS other_id, 0.2::float AS score
        FROM work_projects wp1
        JOIN work_projects wp2 ON wp2.project_id = wp1.project_id AND wp2.work_id <> wp1.work_id
        WHERE wp1.work_id = $1
        LIMIT 32
        "#,
    )
    .bind(seed)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    // 4) any other works in the library (so a fresh import batch still pairs)
    let from_library: Vec<Row> = sqlx::query_as(
        r#"
        SELECT id AS other_id, 0.12::float AS score
        FROM works
        WHERE id <> $1
        ORDER BY created_at DESC
        LIMIT 48
        "#,
    )
    .bind(seed)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    // Works that already have a non-rejected relation with seed (skip re-proposing)
    let already: HashSet<Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT rm2.entity_id
        FROM relation_members rm1
        JOIN relation_members rm2 ON rm2.relation_id = rm1.relation_id AND rm2.entity_id <> rm1.entity_id
        JOIN relations r ON r.id = rm1.relation_id
        WHERE rm1.entity_kind = 'work' AND rm1.entity_id = $1
          AND rm2.entity_kind = 'work'
          AND r.review_status <> 'rejected'
          AND r.type NOT IN ('cites', 'version_of')
        "#,
    )
    .bind(seed)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let mut scores: HashMap<Uuid, f64> = HashMap::new();
    for row in from_neighbors
        .into_iter()
        .chain(from_cites)
        .chain(from_project)
        .chain(from_library)
    {
        if row.other_id == seed || already.contains(&row.other_id) {
            continue;
        }
        scores
            .entry(row.other_id)
            .and_modify(|s| *s = s.max(row.score))
            .or_insert(row.score);
    }

    let mut pairs: Vec<_> = scores.into_iter().collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    pairs.truncate(K);
    Ok(pairs.into_iter().map(|(id, _)| id).collect())
}

async fn relation_exists(
    pool: &sqlx::PgPool,
    rtype: RelationType,
    source: Uuid,
    target: Uuid,
) -> anyhow::Result<bool> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM relations r
        JOIN relation_members s ON s.relation_id = r.id AND s.role = 'source' AND s.entity_id = $2
        JOIN relation_members t ON t.relation_id = r.id AND t.role = 'target' AND t.entity_id = $3
        WHERE r.type = $1 AND r.review_status <> 'rejected'
        "#,
    )
    .bind(rtype)
    .bind(source)
    .bind(target)
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

fn build_pair_prompt(a: &PaperBrief, b: &PaperBrief) -> String {
    fn block(tag: &str, p: &PaperBrief) -> String {
        let methods = if p.methods.is_empty() {
            "(none extracted)".into()
        } else {
            p.methods
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let claims = if p.claims.is_empty() {
            "(none extracted)".into()
        } else {
            p.claims
                .iter()
                .map(|c| format!("- {c}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "### Paper {tag}\ntitle: {}\nabstract: {}\nmethods:\n{}\nclaims:\n{}\n",
            p.title,
            p.abstract_text.chars().take(1500).collect::<String>(),
            methods,
            claims
        )
    }
    format!(
        "Compare paper A and paper B. Propose whitelist relations if any.\n\n{}{}",
        block("A", a),
        block("B", b)
    )
}
