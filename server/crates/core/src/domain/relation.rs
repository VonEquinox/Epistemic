use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    EntityKind, MemberRole, RelationType, ReviewStatus, ReviewVerdict, SourceLayer, SubjectKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Relation {
    pub id: Uuid,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub relation_type: RelationType,
    pub aspect: Option<String>,
    pub scope: Option<String>,
    pub explanation: String,
    pub confidence: Option<f64>,
    pub source: SourceLayer,
    pub review_status: ReviewStatus,
    pub created_by_user: Option<Uuid>,
    pub model_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct RelationMember {
    pub relation_id: Uuid,
    pub entity_kind: EntityKind,
    pub entity_id: Uuid,
    pub role: MemberRole,
    pub anchor_work_id: Option<Uuid>,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct EvidenceSpan {
    pub id: Uuid,
    pub relation_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub extraction_field: Option<String>,
    pub version_id: Uuid,
    pub page: i32,
    pub text: String,
    pub bbox: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Review {
    pub id: Uuid,
    pub subject_kind: SubjectKind,
    pub subject_id: Uuid,
    pub user_id: Uuid,
    pub verdict: ReviewVerdict,
    pub comment: String,
    pub created_at: DateTime<Utc>,
}

/// Full relation with members + evidence for review queue / ego edge panel.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RelationDetail {
    pub relation: Relation,
    pub members: Vec<RelationMember>,
    pub evidence: Vec<EvidenceSpan>,
    pub reviews: Vec<Review>,
}
