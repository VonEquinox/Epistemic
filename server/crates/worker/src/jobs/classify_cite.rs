//! Citation context classification — PRD §6.3 path 1 (primary).
//!
//! Parse legacy TEI (if any) for `<ref type="bibr">` contexts, batch them to the LLM,
//! and write ai_candidate relations with evidence spans for confidence ≥ 0.5.
//! Without TEI (GROBID removed), this job no-ops into propose_pairs.

use super::{version_id, work_id, JobContext};
use epistemic_core::domain::{
    job_kind, EntityKind, Job, MemberRole, RelationType, ReviewStatus, SourceLayer,
};
use epistemic_core::repo::relations::{NewEvidence, NewRelation, NewRelationMember};
use epistemic_core::repo::{jobs, relations, works};
use epistemic_core::util::normalize_title;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use uuid::Uuid;

const PROMPT_VERSION: &str = "cite_ctx_v1";
const MIN_CONFIDENCE: f64 = 0.5;
const BATCH_SIZE: usize = 12;

#[derive(Debug, Clone)]
struct CiteContext {
    id: String,
    target: String,
    text: String,
    page: i32,
    bbox: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct BiblEntry {
    target: String,
    title: Option<String>,
    arxiv_id: Option<String>,
    doi: Option<String>,
}

pub async fn run(ctx: &JobContext, job: &Job) -> anyhow::Result<()> {
    let llm = ctx
        .llm
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;
    let vid = version_id(job)?;
    let version = works::get_version(&ctx.pool, vid).await?;
    let citing_work_id = work_id(job).unwrap_or(version.work_id);

    let tei = load_tei(ctx, &version).await?;
    let Some(tei) = tei else {
        tracing::info!(%vid, "no TEI available; skip citation context classification");
        enqueue_propose(ctx, citing_work_id, vid).await?;
        return Ok(());
    };

    let bibls = parse_bibl_structs(&tei);
    let contexts = parse_cite_contexts(&tei);
    if contexts.is_empty() {
        tracing::info!(%vid, "no citation contexts found in TEI");
        enqueue_propose(ctx, citing_work_id, vid).await?;
        return Ok(());
    }

    let mut target_work: HashMap<String, (Uuid, String)> = HashMap::new();
    for b in &bibls {
        if let Some((wid, title)) = resolve_bibl(ctx, b).await? {
            target_work.insert(b.target.clone(), (wid, title));
        }
    }

    let usable: Vec<&CiteContext> = contexts
        .iter()
        .filter(|c| target_work.contains_key(&c.target))
        .collect();
    tracing::info!(
        total_ctx = contexts.len(),
        in_library = usable.len(),
        bibls = bibls.len(),
        "citation contexts ready"
    );

    if usable.is_empty() {
        enqueue_propose(ctx, citing_work_id, vid).await?;
        return Ok(());
    }

    let system = include_str!("../../../llm/prompts/cite_ctx_v1.md");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../llm/prompts/cite_ctx_schema_v1.json"))?;
    let model_ver = format!("{}/{PROMPT_VERSION}", llm.model());

    let mut created = 0u32;
    for chunk in usable.chunks(BATCH_SIZE) {
        let user = build_user_prompt(&version.title, chunk, &target_work);
        let (value, usage, model) = llm.complete_json(system, &user, schema.clone(), 4000).await?;
        let cost = epistemic_llm::estimate_cost_usd(&model, &usage);
        tracing::info!(
            batch = chunk.len(),
            cost_usd = cost,
            input = usage.input_tokens,
            output = usage.output_tokens,
            "cite_ctx batch done"
        );

        let items = value
            .get("classifications")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for item in items {
            let ctx_id = item
                .get("context_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let type_s = item.get("type").and_then(|v| v.as_str()).unwrap_or("none");
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

            let Some(cctx) = chunk.iter().find(|c| c.id == ctx_id) else {
                continue;
            };
            let Some((cited_wid, _)) = target_work.get(&cctx.target) else {
                continue;
            };
            if *cited_wid == citing_work_id {
                continue;
            }
            if relation_exists(&ctx.pool, rtype, citing_work_id, *cited_wid).await? {
                continue;
            }

            let explanation = item
                .get("explanation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let mut evidence_text = item
                .get("evidence_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if evidence_text.is_empty() {
                evidence_text = cctx.text.clone();
            }
            if evidence_text.trim().is_empty() {
                continue; // principle 2: no evidence → discard
            }

            let aspect = item
                .get("aspect")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let page = if cctx.page > 0 { cctx.page } else { 1 };

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
                            entity_id: citing_work_id,
                            role: MemberRole::Source,
                            anchor_work_id: Some(citing_work_id),
                            position: 0,
                        },
                        NewRelationMember {
                            entity_kind: EntityKind::Work,
                            entity_id: *cited_wid,
                            role: MemberRole::Target,
                            anchor_work_id: Some(*cited_wid),
                            position: 1,
                        },
                    ],
                    evidence: vec![NewEvidence {
                        version_id: vid,
                        page,
                        text: evidence_text,
                        bbox: cctx.bbox.clone(),
                    }],
                },
            )
            .await?;
            created += 1;
        }
    }

    tracing::info!(%vid, created, "citation context classification done");
    enqueue_propose(ctx, citing_work_id, vid).await?;
    Ok(())
}

async fn enqueue_propose(ctx: &JobContext, work_id: Uuid, version_id: Uuid) -> anyhow::Result<()> {
    jobs::enqueue(
        &ctx.pool,
        job_kind::PROPOSE_PAIRS,
        serde_json::json!({ "work_id": work_id, "version_id": version_id }),
    )
    .await?;
    Ok(())
}

async fn load_tei(
    ctx: &JobContext,
    version: &epistemic_core::domain::Version,
) -> anyhow::Result<Option<String>> {
    let Some(ref tei_rel) = version.tei_path else {
        return Ok(None);
    };
    let p = ctx.tei_dir.join(tei_rel);
    if !p.exists() {
        return Ok(None);
    }
    Ok(Some(tokio::fs::read_to_string(&p).await?))
}

async fn resolve_bibl(ctx: &JobContext, b: &BiblEntry) -> anyhow::Result<Option<(Uuid, String)>> {
    if let Some(ref ax) = b.arxiv_id {
        if let Some(v) = works::find_version_by_arxiv(&ctx.pool, ax).await? {
            return Ok(Some((v.work_id, v.title)));
        }
    }
    if let Some(ref doi) = b.doi {
        if let Some(v) = works::find_version_by_doi(&ctx.pool, doi).await? {
            return Ok(Some((v.work_id, v.title)));
        }
    }
    if let Some(ref title) = b.title {
        let tnorm = normalize_title(title);
        if let Some(row) = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT w.id, COALESCE(v.title, w.title_norm)
            FROM works w
            LEFT JOIN versions v ON v.id = w.primary_version_id
            WHERE w.title_norm = $1
            LIMIT 1
            "#,
        )
        .bind(&tnorm)
        .fetch_optional(&ctx.pool)
        .await?
        {
            return Ok(Some(row));
        }
    }
    Ok(None)
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

fn build_user_prompt(
    citing_title: &str,
    chunk: &[&CiteContext],
    target_work: &HashMap<String, (Uuid, String)>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("Citing paper title: {citing_title}\n\n"));
    out.push_str("Classify each citation context below.\n\n");
    for c in chunk {
        let cited_title = target_work
            .get(&c.target)
            .map(|(_, t)| t.as_str())
            .unwrap_or("(unknown)");
        out.push_str(&format!(
            "### context_id={}\npage={}\ncited_title={}\ncontext:\n{}\n\n",
            c.id, c.page, cited_title, c.text
        ));
    }
    out
}

// ─── TEI parsing ─────────────────────────────────────────────────────────────

fn parse_bibl_structs(tei: &str) -> Vec<BiblEntry> {
    let re = bibl_re();
    let mut out = Vec::new();
    for cap in re.captures_iter(tei) {
        let id = cap.name("id").map(|m| m.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let body = cap.name("body").map(|m| m.as_str()).unwrap_or("");
        let title = first_tag_text(body, "title");
        let doi = idno(body, "DOI").or_else(|| idno(body, "doi"));
        let arxiv_id = idno(body, "arXiv")
            .or_else(|| idno(body, "arxiv"))
            .map(|s| s.trim_start_matches("arXiv:").trim().to_string());
        out.push(BiblEntry {
            target: format!("#{id}"),
            title,
            arxiv_id,
            doi,
        });
    }
    out
}

fn parse_cite_contexts(tei: &str) -> Vec<CiteContext> {
    let re = ref_re();
    let mut out = Vec::new();
    let mut idx = 0u32;
    for cap in re.captures_iter(tei) {
        let whole = cap.get(0).unwrap();
        let target = cap
            .name("target")
            .or_else(|| cap.name("target2"))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if target.is_empty() {
            continue;
        }
        // Extract coords from the full match — attribute order varies in GROBID TEI.
        let coords = attr_from_tag(whole.as_str(), "coords");
        let (page, bbox) = parse_coords(coords.as_deref());

        let start = whole.start().saturating_sub(280);
        let end = (whole.end() + 200).min(tei.len());
        let window = &tei[start..end];
        let text = strip_tags(window);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.len() < 20 {
            continue;
        }
        let text: String = text.chars().take(600).collect();

        out.push(CiteContext {
            id: format!("c{idx}"),
            target,
            text,
            page,
            bbox,
        });
        idx += 1;
    }
    out
}

fn attr_from_tag(tag: &str, name: &str) -> Option<String> {
    // Match name="..." inside the opening tag portion (before '>')
    let open_end = tag.find('>').unwrap_or(tag.len());
    let head = &tag[..open_end];
    let key = format!(r#"{name}=""#);
    let start = head.find(&key)? + key.len();
    let rest = &head[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_coords(coords: Option<&str>) -> (i32, Option<serde_json::Value>) {
    let Some(c) = coords else {
        return (0, None);
    };
    let first = c.split(';').next().unwrap_or(c);
    let parts: Vec<f64> = first
        .split_whitespace()
        .filter_map(|p| p.parse().ok())
        .collect();
    if parts.len() >= 5 {
        let page = parts[0] as i32;
        let x0 = parts[1];
        let y0 = parts[2];
        let x1 = parts[3];
        let y1 = parts[4];
        let bbox = serde_json::json!({
            "x": x0, "y": y0, "w": (x1 - x0).max(0.0), "h": (y1 - y0).max(0.0)
        });
        (page, Some(bbox))
    } else if parts.len() == 1 {
        (parts[0] as i32, None)
    } else {
        (0, None)
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn first_tag_text(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = body.find(&open)?;
    let after = &body[start..];
    let gt = after.find('>')?;
    let rest = &after[gt + 1..];
    let end = rest.find(&close)?;
    let raw = &rest[..end];
    let t = strip_tags(raw);
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn idno(body: &str, kind: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"(?is)<idno[^>]*type=["']{}["'][^>]*>(.*?)</idno>"#,
        regex::escape(kind)
    ))
    .ok()?;
    let cap = re.captures(body)?;
    let t = strip_tags(cap.get(1)?.as_str())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("");
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn bibl_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?is)<biblStruct\b[^>]*xml:id=["'](?P<id>[^"']+)["'][^>]*>(?P<body>.*?)</biblStruct>"#,
        )
        .expect("bibl re")
    })
}

fn ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Cover both attribute orders: type-then-target and target-then-type.
        // coords are extracted separately via attr_from_tag (order-independent).
        Regex::new(
            r#"(?is)<ref\b[^>]*type=["']bibr["'][^>]*target=["'](?P<target>#[^"']+)["'][^>]*/?>|<ref\b[^>]*target=["'](?P<target2>#[^"']+)["'][^>]*type=["']bibr["'][^>]*/?>"#,
        )
        .expect("ref re")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bibl_and_refs() {
        let tei = r##"
        <TEI>
          <text>
            <body>
              <p>We build on the Transformer <ref type="bibr" target="#b0" coords="2 100 200 300 220">[1]</ref>
              and compare against LSTM baselines.</p>
            </body>
            <back>
              <listBibl>
                <biblStruct xml:id="b0">
                  <analytic>
                    <title level="a">Attention Is All You Need</title>
                  </analytic>
                  <idno type="arXiv">1706.03762</idno>
                </biblStruct>
              </listBibl>
            </back>
          </text>
        </TEI>
        "##;
        let bibls = parse_bibl_structs(tei);
        assert_eq!(bibls.len(), 1);
        assert_eq!(bibls[0].target, "#b0");
        assert_eq!(bibls[0].arxiv_id.as_deref(), Some("1706.03762"));
        assert!(bibls[0]
            .title
            .as_deref()
            .unwrap_or("")
            .contains("Attention"));

        let ctxs = parse_cite_contexts(tei);
        assert!(!ctxs.is_empty(), "expected at least one context");
        assert_eq!(ctxs[0].target, "#b0");
        assert!(
            ctxs[0].text.contains("Transformer") || ctxs[0].text.contains("build"),
            "text={}",
            ctxs[0].text
        );
        assert_eq!(ctxs[0].page, 2);
    }

    #[test]
    fn coords_parse() {
        let (p, b) = parse_coords(Some("3 10.0 20.0 100.0 40.0"));
        assert_eq!(p, 3);
        assert!(b.is_some());
    }

    #[test]
    fn from_llm_whitelist() {
        assert_eq!(
            RelationType::from_llm("improves_on"),
            Some(RelationType::ImprovesOn)
        );
        assert_eq!(RelationType::from_llm("none"), None);
        assert_eq!(RelationType::from_llm("cites"), None);
    }
}
