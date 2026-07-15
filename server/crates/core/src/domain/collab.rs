use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{AnnotationKind, ClaimVerdict, ReadingLevel, Visibility};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct ClaimJudgment {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub user_id: Uuid,
    pub verdict: ClaimVerdict,
    pub conditions: String,
    pub evidence_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct ReadingStatusRow {
    pub user_id: Uuid,
    pub work_id: Uuid,
    pub status: ReadingLevel,
    pub starred: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Annotation {
    pub id: Uuid,
    pub work_id: Uuid,
    pub version_id: Option<Uuid>,
    pub user_id: Uuid,
    pub kind: AnnotationKind,
    pub visibility: Visibility,
    pub anchor: Option<serde_json::Value>,
    pub body: String,
    pub parent_id: Option<Uuid>,
    pub resolved: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Neighbor {
    pub dimension: super::NeighborDimension,
    pub work_id: Uuid,
    pub neighbor_work_id: Uuid,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct SavedView {
    pub id: Uuid,
    pub name: String,
    pub weights: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
