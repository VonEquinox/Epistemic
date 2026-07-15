use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{ImportStatus, JobStatus};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: serde_json::Value,
    pub status: JobStatus,
    pub attempts: i32,
    pub run_after: DateTime<Utc>,
    pub locked_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Job kind constants (worker matches on these strings).
pub mod job_kind {
    pub const RESOLVE_METADATA: &str = "resolve_metadata";
    pub const FETCH_PDF: &str = "fetch_pdf";
    pub const GROBID_PARSE: &str = "grobid_parse";
    pub const EXTRACT_DNA: &str = "extract_dna";
    pub const FETCH_REFERENCES: &str = "fetch_references";
    pub const UPDATE_NEIGHBORS_CITATION: &str = "update_neighbors_citation";
    pub const UPDATE_NEIGHBORS_LINEAGE: &str = "update_neighbors_lineage";
    pub const CLASSIFY_CITATION_CONTEXTS: &str = "classify_citation_contexts";
    pub const EMBED: &str = "embed";
    pub const PROPOSE_PAIRS: &str = "propose_pairs";
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct ImportBatch {
    pub id: Uuid,
    pub created_by: Option<Uuid>,
    pub raw_input: String,
    pub parsed: Option<serde_json::Value>,
    pub status: ImportStatus,
    pub created_at: DateTime<Utc>,
}
