//! Render PDF pages to PNG via `pdftoppm` (poppler) and pack as data URLs.

use epistemic_llm::image_data_url;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

/// Render every page of `pdf_path` to PNG data URLs (`data:image/png;base64,...`).
///
/// Uses system `pdftoppm`. Resolution via `PDF_RENDER_DPI` (default 150).
/// Optional cap `PDF_MAX_PAGES` (0 / unset = all pages — user requested full PDF).
pub async fn pdf_to_png_data_urls(pdf_path: &Path) -> anyhow::Result<Vec<String>> {
    if !pdf_path.exists() {
        anyhow::bail!("PDF missing: {}", pdf_path.display());
    }
    let dpi: u32 = std::env::var("PDF_RENDER_DPI")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(150);
    let max_pages: Option<u32> = std::env::var("PDF_MAX_PAGES")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0);

    let tmp = tempfile::tempdir()?;
    let prefix = tmp.path().join("page");
    let prefix_str = prefix.to_string_lossy().to_string();

    let mut cmd = Command::new("pdftoppm");
    cmd.arg("-png")
        .arg("-r")
        .arg(dpi.to_string());
    if let Some(n) = max_pages {
        cmd.arg("-f").arg("1").arg("-l").arg(n.to_string());
    }
    cmd.arg(pdf_path)
        .arg(&prefix_str)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let out = cmd.output().await.map_err(|e| {
        anyhow::anyhow!("failed to run pdftoppm (install poppler): {e}")
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("pdftoppm failed: {err}");
    }

    let mut pages: Vec<PathBuf> = std::fs::read_dir(tmp.path())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();
    pages.sort();

    if pages.is_empty() {
        anyhow::bail!("pdftoppm produced no PNG pages for {}", pdf_path.display());
    }

    let mut urls = Vec::with_capacity(pages.len());
    for p in pages {
        let bytes = tokio::fs::read(&p).await?;
        urls.push(image_data_url("image/png", &bytes));
    }
    tracing::info!(
        pages = urls.len(),
        dpi,
        path = %pdf_path.display(),
        "PDF rendered to PNG for VLM"
    );
    Ok(urls)
}
