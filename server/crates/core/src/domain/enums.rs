use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Admin,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "version_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Arxiv,
    Conference,
    Journal,
    Preprint,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "source_layer", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SourceLayer {
    PublicFact,
    TeamRecord,
    AiCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "review_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Unreviewed,
    Confirmed,
    Disputed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "relation_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    Cites,
    VersionOf,
    UsesMethodFrom,
    ImprovesOn,
    AlternativeTo,
    UsesDatasetFrom,
    ComparesAgainst,
    Reproduces,
    FailsToReproduce,
    SupportsClaim,
    ContradictsClaim,
    PrerequisiteFor,
}

impl RelationType {
    pub fn is_high_risk(self) -> bool {
        matches!(self, Self::FailsToReproduce | Self::ContradictsClaim)
    }

    pub fn is_method_lineage(self) -> bool {
        matches!(
            self,
            Self::UsesMethodFrom | Self::ImprovesOn | Self::AlternativeTo
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "entity_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Work,
    Claim,
    Method,
    Dataset,
    Version,
    Person,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "member_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Source,
    Target,
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "review_verdict", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Agree,
    Disagree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "subject_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Relation,
    ClaimJudgment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "claim_verdict", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ClaimVerdict {
    Supported,
    PartiallySupported,
    Contradicted,
    NotReproduced,
    Concern,
    Unclear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "reading_level", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReadingLevel {
    Unread,
    Skimmed,
    Read,
    Reproduced,
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "annotation_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    Note,
    Conjecture,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "visibility", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "job_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "neighbor_dimension", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum NeighborDimension {
    CitationCoupling,
    MethodLineage,
    Topic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "import_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Preview,
    Confirmed,
    Processing,
    Done,
    Failed,
}
