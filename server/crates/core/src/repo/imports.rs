use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{ImportBatch, ImportStatus};
use crate::error::{AppError, AppResult};
use crate::util::{parse_arxiv_id, parse_doi};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ParsedImportLine {
    pub raw: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub arxiv_id: Option<String>,
    pub doi: Option<String>,
    pub error: Option<String>,
}

pub fn parse_import_text(raw: &str) -> Vec<ParsedImportLine> {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let mut title = None;
            let mut url = None;
            let mut arxiv_id = None;
            let mut doi = None;
            let mut error = None;

            let parts: Vec<&str> = if line.contains('|') {
                line.splitn(2, '|').map(|s| s.trim()).collect()
            } else if line.contains('\t') {
                line.splitn(2, '\t').map(|s| s.trim()).collect()
            } else if let Some(idx) = line.find("http") {
                if idx > 0 {
                    vec![line[..idx].trim_end_matches([',', ' ', ';']), &line[idx..]]
                } else {
                    vec![line]
                }
            } else {
                vec![line]
            };

            match parts.len() {
                1 => {
                    let token = parts[0];
                    if let Some(id) = parse_arxiv_id(token) {
                        arxiv_id = Some(id);
                        url = Some(token.to_string());
                    } else if let Some(d) = parse_doi(token) {
                        doi = Some(d);
                        url = Some(token.to_string());
                    } else if token.starts_with("http") {
                        url = Some(token.to_string());
                        arxiv_id = parse_arxiv_id(token);
                        doi = parse_doi(token);
                        if arxiv_id.is_none() && doi.is_none() {
                            error = Some("unrecognized URL; metadata will be best-effort".into());
                        }
                    } else {
                        title = Some(token.to_string());
                        error = Some("no identifier; needs manual URL/DOI".into());
                    }
                }
                2 => {
                    title = Some(parts[0].to_string());
                    let token = parts[1];
                    url = Some(token.to_string());
                    arxiv_id = parse_arxiv_id(token);
                    doi = parse_doi(token);
                }
                _ => error = Some("could not parse line".into()),
            }

            ParsedImportLine {
                raw: line.to_string(),
                title,
                url,
                arxiv_id,
                doi,
                error,
            }
        })
        .collect()
}

pub async fn create_batch(
    pool: &PgPool,
    created_by: Uuid,
    raw_input: &str,
) -> AppResult<ImportBatch> {
    let parsed = parse_import_text(raw_input);
    let parsed_json =
        serde_json::to_value(&parsed).map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let batch = sqlx::query_as::<_, ImportBatch>(
        r#"
        INSERT INTO import_batches (created_by, raw_input, parsed, status)
        VALUES ($1, $2, $3, 'preview')
        RETURNING id, created_by, raw_input, parsed, status, created_at
        "#,
    )
    .bind(created_by)
    .bind(raw_input)
    .bind(parsed_json)
    .fetch_one(pool)
    .await?;
    Ok(batch)
}

pub async fn get_batch(pool: &PgPool, id: Uuid) -> AppResult<ImportBatch> {
    sqlx::query_as::<_, ImportBatch>(
        r#"
        SELECT id, created_by, raw_input, parsed, status, created_at
        FROM import_batches WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("import batch {id}")))
}

pub async fn set_status(pool: &PgPool, id: Uuid, status: ImportStatus) -> AppResult<()> {
    sqlx::query(r#"UPDATE import_batches SET status = $2 WHERE id = $1"#)
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_arxiv_lines() {
        let text = r#"
# comment
https://arxiv.org/abs/1706.03762
Attention Is All You Need | https://arxiv.org/pdf/1706.03762.pdf
10.1038/nature14539
"#;
        let lines = parse_import_text(text);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].arxiv_id.as_deref(), Some("1706.03762"));
        assert_eq!(lines[1].title.as_deref(), Some("Attention Is All You Need"));
        assert_eq!(lines[2].doi.as_deref(), Some("10.1038/nature14539"));
    }
}
