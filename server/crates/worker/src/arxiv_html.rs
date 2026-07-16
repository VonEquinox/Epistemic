//! arXiv HTML (experimental) full-paper fetch + plain-text extraction.
//! Source of truth for DNA/metadata when export.arxiv.org API is rate-limited.

use anyhow::{anyhow, Context};
use regex::Regex;
use std::sync::OnceLock;

const UA: &str = "EpistemicWorker/0.1 (research library; contact: admin@example.com)";

#[derive(Debug, Clone)]
pub struct ArxivHtmlDoc {
    pub url: String,
    pub html: String,
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub authors: Vec<String>,
    /// Cleaned full paper text (body), as complete as HTML allows.
    pub full_text: String,
}

/// Fetch HTML experimental page for an arXiv id (`2404.14387` or `2404.14387v1`).
pub async fn fetch_arxiv_html(
    http: &reqwest::Client,
    arxiv_id: &str,
) -> anyhow::Result<ArxivHtmlDoc> {
    let id = arxiv_id.trim().trim_start_matches("arXiv:").trim();
    let candidates = [
        format!("https://arxiv.org/html/{id}"),
        format!("https://ar5iv.labs.arxiv.org/html/{id}"),
    ];

    let mut last_err = None;
    for url in &candidates {
        tracing::info!(%url, "fetching arXiv HTML");
        match http
            .get(url)
            .header("User-Agent", UA)
            .header("Accept", "text/html,application/xhtml+xml")
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_err = Some(anyhow!("HTTP {status} for {url}"));
                    continue;
                }
                let html = resp.text().await.context("read html body")?;
                if html.len() < 500
                    || html.contains("HTML is not available")
                    || html.contains("not available for this paper")
                {
                    last_err = Some(anyhow!("HTML unavailable or too short for {url}"));
                    continue;
                }
                let (title, abs, authors, full_text) = extract_from_html(&html);
                if full_text.chars().count() < 200 {
                    last_err = Some(anyhow!("extracted text too short from {url}"));
                    continue;
                }
                return Ok(ArxivHtmlDoc {
                    url: url.clone(),
                    html,
                    title,
                    abstract_text: abs,
                    authors,
                    full_text,
                });
            }
            Err(e) => {
                last_err = Some(e.into());
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("no HTML candidate worked for {id}")))
}

fn extract_from_html(html: &str) -> (Option<String>, Option<String>, Vec<String>, String) {
    let title = {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());
        re.captures(html)
            .and_then(|c| c.get(1).map(|m| decode_entities(m.as_str())))
            .map(|s| {
                s.replace(" | arXiv", "")
                    .replace(" – ar5iv", "")
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty())
    };

    // Prefer article / page content slice.
    let body_html = slice_main_content(html);

    let abstract_text = extract_abstract(body_html);

    let authors = extract_authors(body_html);

    let full_text = html_to_text(body_html);

    (title, abstract_text, authors, full_text)
}

fn slice_main_content(html: &str) -> &str {
    // LaTeXML / ar5iv main content markers.
    if let Some(i) = html.find(r#"class="ltx_page_content""#) {
        let rest = &html[i..];
        if let Some(j) = rest.find("<footer") {
            return &rest[..j];
        }
        return rest;
    }
    if let Some(i) = html.find(r#"class="ltx_document""#) {
        return &html[i..];
    }
    if let Some(i) = html.find("<article") {
        return &html[i..];
    }
    html
}

fn extract_abstract(body: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?is)<div[^>]*class="[^"]*ltx_abstract[^"]*"[^>]*>(.*?)</div>"#).unwrap()
    });
    if let Some(c) = re.captures(body) {
        let t = html_to_text(c.get(1)?.as_str());
        let t = t
            .trim_start_matches("Abstract")
            .trim_start_matches('.')
            .trim()
            .to_string();
        if t.len() > 40 {
            return Some(t);
        }
    }
    None
}

fn extract_authors(body: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?is)<span[^>]*class="[^"]*ltx_personname[^"]*"[^>]*>(.*?)</span>"#).unwrap()
    });
    let mut out = Vec::new();
    for cap in re.captures_iter(body) {
        let name = html_to_text(cap.get(1).map(|m| m.as_str()).unwrap_or(""))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if name.len() >= 2 && !out.iter().any(|x: &String| x == &name) {
            out.push(name);
        }
        if out.len() >= 40 {
            break;
        }
    }
    out
}

/// Strip tags → plain text; keep img alts; preserve paragraph breaks.
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    // drop scripts/styles/nav/svg
    for pat in [
        r"(?is)<script\b[^>]*>.*?</script>",
        r"(?is)<style\b[^>]*>.*?</style>",
        r"(?is)<noscript\b[^>]*>.*?</noscript>",
        r"(?is)<svg\b[^>]*>.*?</svg>",
        r"(?is)<nav\b[^>]*>.*?</nav>",
        r"(?is)<!--.*?-->",
    ] {
        if let Ok(re) = Regex::new(pat) {
            s = re.replace_all(&s, " ").into_owned();
        }
    }
    // images → [IMAGE: alt or src]
    if let Ok(re) = Regex::new(r#"(?is)<img\b([^>]*)>"#) {
        s = re
            .replace_all(&s, |caps: &regex::Captures| {
                let attrs = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let alt = attr(attrs, "alt").unwrap_or_default();
                let src = attr(attrs, "src").unwrap_or_default();
                let label = if !alt.is_empty() {
                    alt
                } else if !src.is_empty() {
                    src.rsplit('/').next().unwrap_or(&src).to_string()
                } else {
                    "figure".into()
                };
                format!("\n[IMAGE: {label}]\n")
            })
            .into_owned();
    }
    // block ends → newlines
    if let Ok(re) = Regex::new(
        r"(?is)</(p|div|h1|h2|h3|h4|h5|h6|li|tr|section|article|figcaption|blockquote|pre|br)\s*>",
    ) {
        s = re.replace_all(&s, "\n").into_owned();
    }
    if let Ok(re) = Regex::new(r"(?i)<br\s*/?>") {
        s = re.replace_all(&s, "\n").into_owned();
    }
    // strip remaining tags
    if let Ok(re) = Regex::new(r"(?s)<[^>]+>") {
        s = re.replace_all(&s, " ").into_owned();
    }
    s = decode_entities(&s);
    // normalize whitespace
    let mut out = String::with_capacity(s.len());
    let mut prev_nl = 0u8;
    let mut prev_space = false;
    for ch in s.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            if prev_nl < 2 {
                out.push('\n');
                prev_nl += 1;
            }
            prev_space = false;
            continue;
        }
        if ch.is_whitespace() {
            if !prev_space && prev_nl == 0 {
                out.push(' ');
                prev_space = true;
            }
            continue;
        }
        out.push(ch);
        prev_space = false;
        prev_nl = 0;
    }
    out.trim().to_string()
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"(?i){}\s*=\s*"([^"]*)""#, regex::escape(name))).ok()?;
    re.captures(attrs)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .or_else(|| {
            let re = Regex::new(&format!(r#"(?i){}\s*=\s*'([^']*)'"#, regex::escape(name))).ok()?;
            re.captures(attrs)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        })
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&middot;", "·")
        .replace("&times;", "×")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags() {
        let t = html_to_text("<p>Hello <b>world</b></p><p>Next</p>");
        assert!(t.contains("Hello world"));
        assert!(t.contains("Next"));
    }
}
