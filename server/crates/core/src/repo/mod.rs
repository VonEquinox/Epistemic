pub mod users;
pub mod works;
pub mod projects;
pub mod reading;
pub mod jobs;
pub mod imports;
pub mod relations;
pub mod annotations;
pub mod graph;

// Explicit re-exports to avoid name clashes (get / list_for_work).
pub use users::{
    authenticate, bootstrap_admin, create_invite, create_user, find_by_email, find_by_id,
    find_invite_by_token, list_users, mark_invite_used,
};
pub use works::{
    authors_for_version, create_or_get_work, find_version_by_arxiv, find_version_by_doi,
    get_version, get_work, get_work_card, list_claims, list_methods, list_versions_for_work,
    list_works, merge_works, update_version_metadata, update_version_paths, NewVersion,
    WorkListItem, WorkListQuery,
};
pub use projects::{
    attach_work, create_project, get_project, list_projects, project_coverage, projects_for_work,
    CoverageEntry, CoverageReader,
};
pub use jobs::{
    claim_next, enqueue, enqueue_many, jobs_for_version, mark_done, mark_failed,
};
pub use imports::{create_batch, get_batch, parse_import_text, set_status, ParsedImportLine};
pub use relations::{
    add_review, create_relation, get_relation, patch_relation, review_queue, set_review_status,
    NewEvidence, NewRelation, NewRelationMember, ReviewQueueQuery,
};
pub use graph::{
    ego_work, map_data, trim_neighbors, upsert_neighbor, EgoEdge, EgoNode, EgoResponse, MapNode,
    MapResponse, NeighborEntry,
};
