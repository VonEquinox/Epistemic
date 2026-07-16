pub mod annotations;
pub mod aspects;
pub mod claims;
pub mod comments;
pub mod evidence;
pub mod graph;
pub mod groups;
pub mod imports;
pub mod jobs;
pub mod projects;
pub mod reading;
pub mod relations;
pub mod users;
pub mod views;
pub mod works;

// Explicit re-exports to avoid name clashes (get / list_for_work).
pub use claims::{
    add_judgment, add_judgment_review, create as create_claim, get_judgment, get_judgment_detail,
    get_with_evidence as get_claim, list_judgments, list_judgments_detailed,
    list_with_evidence_for_work, promote_from_selection, ClaimJudgmentDetail, ClaimWithEvidence,
    NewClaim,
};
pub use evidence::{
    create as create_evidence, get as get_evidence, list_for_claim as list_evidence_for_claim,
    list_for_relation as list_evidence_for_relation, list_for_version as list_evidence_for_version,
    list_for_work as list_evidence_for_work, NewEvidenceSpan,
};
pub use graph::{
    ego_work, ego_work_mode, map_data, trim_neighbors, upsert_neighbor, EgoEdge, EgoGroup, EgoNode,
    EgoResponse, MapEdge, MapNode, MapResponse, NeighborEntry,
};
pub use imports::{
    begin_confirm, create_batch, get_batch, get_batch_for_user, parse_import_text, set_status,
    ParsedImportLine,
};
pub use jobs::{
    claim_next, enqueue, enqueue_many, enqueue_tx, enqueue_unique, enqueue_unique_tx,
    jobs_for_version, jobs_for_work, mark_done, mark_failed, requeue, reschedule,
};
pub use projects::{
    attach_work, create_project, get_project, list_projects, project_coverage, projects_for_work,
    CoverageEntry, CoverageReader,
};
pub use relations::{
    add_review, create_relation, get_relation, list_for_work as list_relations_for_work,
    list_reviews, patch_relation, recompute_status, review_queue, set_review_status, upsert_review,
    NewEvidence, NewRelation, NewRelationMember, ReviewQueueQuery,
};
pub use users::{
    authenticate, bootstrap_admin, create_invite, create_user, find_by_email, find_by_id,
    find_invite_by_token, list_users, mark_invite_used,
};
pub use views::{
    create as create_view, delete as delete_view, get as get_view, list as list_views, SavedView,
};
pub use works::{
    authors_for_version, create_or_get_work, find_version_by_arxiv, find_version_by_doi,
    get_version, get_work, get_work_card, list_claims, list_methods, list_versions_for_work,
    list_works, merge_works, split_work, update_version_metadata, update_version_paths, NewVersion,
    WorkListItem, WorkListQuery,
};
