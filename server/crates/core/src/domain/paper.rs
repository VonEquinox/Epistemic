use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{UserRole, VersionKind};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            role: u.role,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Invite {
    pub id: Uuid,
    pub email: String,
    pub token: String,
    pub created_by: Uuid,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Work {
    pub id: Uuid,
    pub title_norm: String,
    pub primary_version_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Version {
    pub id: Uuid,
    pub work_id: Uuid,
    pub kind: VersionKind,
    pub arxiv_id: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub title: String,
    pub abstract_text: String,
    pub year: Option<i32>,
    pub venue_name: Option<String>,
    pub pdf_path: Option<String>,
    pub tei_path: Option<String>,
    pub metadata_source: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Row mapping when the DB column is named `abstract` (reserved word).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct VersionRow {
    pub id: Uuid,
    pub work_id: Uuid,
    pub kind: VersionKind,
    pub arxiv_id: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub title: String,
    #[sqlx(rename = "abstract")]
    pub abstract_text: String,
    pub year: Option<i32>,
    pub venue_name: Option<String>,
    pub pdf_path: Option<String>,
    pub tei_path: Option<String>,
    pub metadata_source: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<VersionRow> for Version {
    fn from(r: VersionRow) -> Self {
        Self {
            id: r.id,
            work_id: r.work_id,
            kind: r.kind,
            arxiv_id: r.arxiv_id,
            doi: r.doi,
            url: r.url,
            title: r.title,
            abstract_text: r.abstract_text,
            year: r.year,
            venue_name: r.venue_name,
            pdf_path: r.pdf_path,
            tei_path: r.tei_path,
            metadata_source: r.metadata_source,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Author {
    pub id: Uuid,
    pub full_name: String,
    pub s2_author_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VersionAuthor {
    pub author: Author,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Citation {
    pub id: Uuid,
    pub citing_work_id: Uuid,
    pub cited_work_id: Option<Uuid>,
    pub cited_external: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Aggregated paper card payload (GET /works/{id}).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkCard {
    pub work: Work,
    pub primary_version: Option<Version>,
    pub versions: Vec<Version>,
    pub authors: Vec<VersionAuthor>,
    pub projects: Vec<Project>,
    pub claims: Vec<super::Claim>,
    pub methods: Vec<super::Method>,
    pub reading: Vec<super::ReadingStatusRow>,
    pub annotations_count: i64,
    pub evidence: Vec<super::EvidenceSpan>,
}
