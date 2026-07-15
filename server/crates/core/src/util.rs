use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use regex::Regex;
use std::sync::OnceLock;

/// Normalize a paper title for fuzzy matching / dedup.
pub fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract arXiv id from a URL or bare id string.
/// Supports: abs/pdf links, arxiv: prefix, bare ids with optional version.
pub fn parse_arxiv_id(input: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:arxiv\.org/(?:abs|pdf)/|arxiv:)?(\d{4}\.\d{4,5})(?:v\d+)?(?:\.pdf)?",
        )
        .expect("arxiv regex")
    });
    re.captures(input.trim())
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Extract DOI from a URL or bare DOI string.
pub fn parse_doi(input: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)(?:doi\.org/|doi:)?(10\.\d{4,9}/[-._;()/:A-Z0-9]+)").expect("doi regex")
    });
    re.captures(input.trim())
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Generate a random invite / session-safe token (hex).
pub fn random_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arxiv_from_url() {
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/abs/1706.03762").as_deref(),
            Some("1706.03762")
        );
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/pdf/1706.03762v7.pdf").as_deref(),
            Some("1706.03762")
        );
        assert_eq!(parse_arxiv_id("1706.03762").as_deref(), Some("1706.03762"));
        assert_eq!(parse_arxiv_id("arxiv:1706.03762v1").as_deref(), Some("1706.03762"));
    }

    #[test]
    fn doi_from_url() {
        assert_eq!(
            parse_doi("https://doi.org/10.1038/nature14539").as_deref(),
            Some("10.1038/nature14539")
        );
        assert_eq!(
            parse_doi("10.1000/182").as_deref(),
            Some("10.1000/182")
        );
    }

    #[test]
    fn title_norm() {
        assert_eq!(
            normalize_title("  Attention Is All You Need!  "),
            "attention is all you need"
        );
    }

    #[test]
    fn password_roundtrip() {
        let h = hash_password("secret").unwrap();
        assert!(verify_password("secret", &h));
        assert!(!verify_password("wrong", &h));
    }
}
