use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{ReviewStatus, SourceLayer};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Claim {
    pub id: Uuid,
    pub work_id: Uuid,
    pub text: String,
    pub source: SourceLayer,
    pub review_status: ReviewStatus,
    pub created_by: Option<Uuid>,
    pub model_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Method {
    pub id: Uuid,
    pub work_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub source: SourceLayer,
    pub review_status: ReviewStatus,
    pub created_by: Option<Uuid>,
    pub model_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Dataset {
    pub id: Uuid,
    pub name: String,
    pub aliases: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Extraction {
    pub id: Uuid,
    pub version_id: Uuid,
    pub model: String,
    pub prompt_version: String,
    pub raw: Option<serde_json::Value>,
    pub status: String,
    pub usage: Option<serde_json::Value>,
    pub cost_usd: Option<f64>,
    pub created_at: DateTime<Utc>,
}

/// One fixed analysis layer for a work (multi-aspect DNA).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct PaperAspect {
    pub work_id: Uuid,
    pub aspect: String,
    pub summary: String,
    pub bullets: serde_json::Value,
    pub source_text: String,
    pub page: i32,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
