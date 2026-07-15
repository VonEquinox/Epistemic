use epistemic_core::domain::Version;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct PaperMeta {
    pub title: String,
    pub abstract_text: String,
    pub year: Option<i32>,
    pub venue_name: Option<String>,
    pub doi: Option<String>,
    pub authors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RefItem {
    pub title: Option<String>,
    pub arxiv_id: Option<String>,
    pub doi: Option<String>,
    pub year: Option<i32>,
}

pub async fn fetch_arxiv(http: &reqwest::Client, arxiv_id: &str) -> anyhow::Result<Option<PaperMeta>> {
    let url = format!("http://export.arxiv.org/api/query?id_list={arxiv_id}");
    let text = http.get(&url).send().await?.error_for_status()?.text().await?;

    // Minimal Atom parse
    let title = capture_tag(&text, "title")
        .map(|s| s.lines().skip(1).collect::<Vec<_>>().join(" ").trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| capture_tag(&text, "title"));
    let summary = capture_tag(&text, "summary").map(|s| {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    });
    let published = capture_tag(&text, "published");
    let year = published
        .as_ref()
        .and_then(|p| p.get(0..4))
        .and_then(|y| y.parse().ok());

    let authors: Vec<String> = text
        .split("<author>")
        .skip(1)
        .filter_map(|chunk| capture_tag(chunk, "name"))
        .collect();

    let doi = text
        .split("arxiv:doi")
        .nth(1)
        .and_then(|s| s.split('>').nth(1))
        .and_then(|s| s.split('<').next())
        .map(|s| s.trim().to_string());

    match title {
        Some(title) => Ok(Some(PaperMeta {
            title,
            abstract_text: summary.unwrap_or_default(),
            year,
            venue_name: Some("arXiv".into()),
            doi,
            authors,
        })),
        None => Ok(None),
    }
}

fn capture_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let after = &xml[start..];
    let content_start = after.find('>')? + 1;
    let content = &after[content_start..];
    let end = content.find(&close)?;
    Some(content[..end].trim().to_string())
}

#[derive(Debug, Deserialize)]
struct S2Paper {
    title: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    year: Option<i32>,
    venue: Option<String>,
    #[serde(rename = "externalIds")]
    external_ids: Option<S2ExternalIds>,
    authors: Option<Vec<S2Author>>,
}

#[derive(Debug, Deserialize)]
struct S2ExternalIds {
    #[serde(rename = "DOI")]
    doi: Option<String>,
    #[serde(rename = "ArXiv")]
    arxiv: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S2Author {
    name: Option<String>,
}

pub async fn fetch_s2(
    http: &reqwest::Client,
    api_key: Option<&str>,
    version: &Version,
) -> anyhow::Result<Option<PaperMeta>> {
    let id = if let Some(ref ax) = version.arxiv_id {
        format!("ARXIV:{ax}")
    } else if let Some(ref doi) = version.doi {
        format!("DOI:{doi}")
    } else {
        return Ok(None);
    };

    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/{id}?fields=title,abstract,year,venue,externalIds,authors"
    );
    let mut req = http.get(&url);
    if let Some(key) = api_key {
        req = req.header("x-api-key", key);
    }
    let resp = req.send().await?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("S2 {status}: {body}");
    }
    let paper: S2Paper = resp.json().await?;
    Ok(Some(PaperMeta {
        title: paper.title.unwrap_or_else(|| version.title.clone()),
        abstract_text: paper.abstract_text.unwrap_or_default(),
        year: paper.year,
        venue_name: paper.venue,
        doi: paper.external_ids.and_then(|e| e.doi),
        authors: paper
            .authors
            .unwrap_or_default()
            .into_iter()
            .filter_map(|a| a.name)
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct S2Refs {
    data: Option<Vec<S2RefItem>>,
}

#[derive(Debug, Deserialize)]
struct S2RefItem {
    #[serde(rename = "citedPaper")]
    cited_paper: Option<S2Paper>,
}

pub async fn fetch_references(
    http: &reqwest::Client,
    api_key: Option<&str>,
    arxiv_id: Option<&str>,
    doi: Option<&str>,
) -> anyhow::Result<Vec<RefItem>> {
    let id = if let Some(ax) = arxiv_id {
        format!("ARXIV:{ax}")
    } else if let Some(d) = doi {
        format!("DOI:{d}")
    } else {
        return Ok(vec![]);
    };

    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/{id}/references?fields=title,year,externalIds&limit=100"
    );
    let mut req = http.get(&url);
    if let Some(key) = api_key {
        req = req.header("x-api-key", key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "S2 references failed");
        return Ok(vec![]);
    }
    let body: S2Refs = resp.json().await?;
    Ok(body
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let p = item.cited_paper?;
            Some(RefItem {
                title: p.title,
                arxiv_id: p.external_ids.as_ref().and_then(|e| e.arxiv.clone()),
                doi: p.external_ids.and_then(|e| e.doi),
                year: p.year,
            })
        })
        .collect())
}
