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

    /// Snake-case wire form used in LLM schemas and DB enums.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cites => "cites",
            Self::VersionOf => "version_of",
            Self::UsesMethodFrom => "uses_method_from",
            Self::ImprovesOn => "improves_on",
            Self::AlternativeTo => "alternative_to",
            Self::UsesDatasetFrom => "uses_dataset_from",
            Self::ComparesAgainst => "compares_against",
            Self::Reproduces => "reproduces",
            Self::FailsToReproduce => "fails_to_reproduce",
            Self::SupportsClaim => "supports_claim",
            Self::ContradictsClaim => "contradicts_claim",
            Self::PrerequisiteFor => "prerequisite_for",
        }
    }

    /// Parse whitelist type from LLM output. Rejects unknown / "none" / "cites".
    pub fn from_llm(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "uses_method_from" => Some(Self::UsesMethodFrom),
            "improves_on" => Some(Self::ImprovesOn),
            "alternative_to" => Some(Self::AlternativeTo),
            "uses_dataset_from" => Some(Self::UsesDatasetFrom),
            "compares_against" => Some(Self::ComparesAgainst),
            "reproduces" => Some(Self::Reproduces),
            "fails_to_reproduce" => Some(Self::FailsToReproduce),
            "supports_claim" => Some(Self::SupportsClaim),
            "contradicts_claim" => Some(Self::ContradictsClaim),
            "prerequisite_for" => Some(Self::PrerequisiteFor),
            // cites / version_of / none are not AI-proposed semantic types here
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_type_as_str_roundtrip_whitelist() {
        for t in [
            RelationType::UsesMethodFrom,
            RelationType::ImprovesOn,
            RelationType::FailsToReproduce,
            RelationType::ContradictsClaim,
            RelationType::PrerequisiteFor,
        ] {
            let s = t.as_str();
            assert_eq!(RelationType::from_llm(s), Some(t));
        }
    }

    #[test]
    fn from_llm_rejects_none_and_cites() {
        assert_eq!(RelationType::from_llm("none"), None);
        assert_eq!(RelationType::from_llm("cites"), None);
        assert_eq!(RelationType::from_llm("version_of"), None);
        assert_eq!(RelationType::from_llm("bogus"), None);
    }

    #[test]
    fn high_risk_flags() {
        assert!(RelationType::FailsToReproduce.is_high_risk());
        assert!(RelationType::ContradictsClaim.is_high_risk());
        assert!(!RelationType::ImprovesOn.is_high_risk());
    }

    #[test]
    fn method_lineage_flags() {
        assert!(RelationType::UsesMethodFrom.is_method_lineage());
        assert!(RelationType::ImprovesOn.is_method_lineage());
        assert!(RelationType::AlternativeTo.is_method_lineage());
        assert!(!RelationType::SupportsClaim.is_method_lineage());
    }
}
