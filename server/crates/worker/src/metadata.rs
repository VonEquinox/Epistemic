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

/// Official arXiv Atom API (not HTML scraping).
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

/// Extract bibliography entries from legacy TEI XML (if present on disk).
/// Not produced by the current pipeline (GROBID removed).
pub fn references_from_tei(tei: &str) -> Vec<RefItem> {
    let mut out = Vec::new();
    let marker = "<biblStruct";
    let mut rest = tei;
    while let Some(start) = rest.find(marker) {
        let chunk = &rest[start..];
        let Some(end_rel) = chunk.find("</biblStruct>") else {
            break;
        };
        let block = &chunk[..end_rel + "</biblStruct>".len()];
        rest = &chunk[end_rel + "</biblStruct>".len()..];

        let title = first_tag_text(block, "title");
        let doi = idno(block, "DOI").or_else(|| idno(block, "doi"));
        let arxiv_id = idno(block, "arXiv")
            .or_else(|| idno(block, "arxiv"))
            .map(|s| s.trim_start_matches("arXiv:").trim().to_string());
        let year = date_year(block);

        if title.is_none() && arxiv_id.is_none() && doi.is_none() {
            continue;
        }
        out.push(RefItem {
            title,
            arxiv_id,
            doi,
            year,
        });
    }
    out
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
    // Match <idno ... type="KIND" ...>value</idno> without regex.
    let lower = body.to_ascii_lowercase();
    let kind_l = kind.to_ascii_lowercase();
    let needle = format!("type=\"{kind_l}\"");
    let needle2 = format!("type='{kind_l}'");
    let pos = lower
        .find(&needle)
        .or_else(|| lower.find(&needle2))?;
    let head = &body[..pos];
    let idno_start = head.rfind("<idno")?;
    let after_open = body[idno_start..].find('>')? + idno_start + 1;
    let close = body[after_open..].find("</idno>")? + after_open;
    let t = strip_tags(&body[after_open..close])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("");
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn date_year(body: &str) -> Option<i32> {
    // <date when="2017"> or bare year in imprint
    if let Some(i) = body.find("when=\"") {
        let rest = &body[i + 6..];
        if let Some(y) = rest.get(0..4).and_then(|s| s.parse().ok()) {
            return Some(y);
        }
    }
    if let Some(i) = body.find("when='") {
        let rest = &body[i + 6..];
        if let Some(y) = rest.get(0..4).and_then(|s| s.parse().ok()) {
            return Some(y);
        }
    }
    let text = strip_tags(body);
    for token in text.split(|c: char| !c.is_ascii_digit()) {
        if token.len() == 4 {
            if let Ok(y) = token.parse::<i32>() {
                if (1900..2100).contains(&y) {
                    return Some(y);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tei_bibl_refs() {
        let tei = r##"
        <listBibl>
          <biblStruct xml:id="b0">
            <analytic>
              <title level="a">Attention Is All You Need</title>
            </analytic>
            <monogr><imprint><date type="published" when="2017" /></imprint></monogr>
            <idno type="arXiv">1706.03762</idno>
          </biblStruct>
          <biblStruct xml:id="b1">
            <monogr>
              <title level="m">Adam: A Method for Stochastic Optimization</title>
              <idno type="DOI">10.48550/arXiv.1412.6980</idno>
            </monogr>
          </biblStruct>
        </listBibl>
        "##;
        let refs = references_from_tei(tei);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].arxiv_id.as_deref(), Some("1706.03762"));
        assert_eq!(refs[0].year, Some(2017));
        assert!(refs[0]
            .title
            .as_deref()
            .unwrap_or("")
            .contains("Attention"));
        assert_eq!(
            refs[1].doi.as_deref(),
            Some("10.48550/arXiv.1412.6980")
        );
    }
}
