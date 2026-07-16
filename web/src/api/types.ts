export type UserRole = 'admin' | 'member';
export type ReadingLevel = 'unread' | 'skimmed' | 'read' | 'reproduced' | 'needs_review';
export type ReviewStatus = 'unreviewed' | 'confirmed' | 'disputed' | 'rejected';
export type SourceLayer = 'public_fact' | 'team_record' | 'ai_candidate';
export type RelationType =
  | 'cites'
  | 'version_of'
  | 'uses_method_from'
  | 'improves_on'
  | 'alternative_to'
  | 'uses_dataset_from'
  | 'compares_against'
  | 'reproduces'
  | 'fails_to_reproduce'
  | 'supports_claim'
  | 'contradicts_claim'
  | 'prerequisite_for';

export type JobStatus = 'queued' | 'running' | 'done' | 'failed';

export type ClaimVerdict =
  | 'supported'
  | 'partially_supported'
  | 'contradicted'
  | 'not_reproduced'
  | 'concern'
  | 'unclear';

export type AnnotationKind = 'note' | 'conjecture' | 'question';
export type Visibility = 'private' | 'team';

export interface User {
  id: string;
  email: string;
  name: string;
  role: UserRole;
  created_at: string;
}


export interface Invite {
  id: string;
  email: string;
  token: string;
  created_by: string;
  used_at?: string | null;
  created_at: string;
}

export interface Work {
  id: string;
  title_norm: string;
  primary_version_id?: string | null;
  created_by?: string | null;
  created_at: string;
}

export interface Version {
  id: string;
  work_id: string;
  kind: string;
  arxiv_id?: string | null;
  doi?: string | null;
  url?: string | null;
  title: string;
  abstract_text: string;
  year?: number | null;
  venue_name?: string | null;
  pdf_path?: string | null;
  tei_path?: string | null;
  metadata_source?: string | null;
  created_at: string;
}

export interface WorkListItem {
  work: Work;
  title: string;
  year?: number | null;
  arxiv_id?: string | null;
  venue_name?: string | null;
  authors: string[];
}

export interface Claim {
  id: string;
  work_id: string;
  text: string;
  source: SourceLayer;
  review_status: ReviewStatus;
  created_at: string;
  /** Present when loaded with judgments (claims-full). */
  judgments?: ClaimJudgment[];
}

export interface Method {
  id: string;
  work_id?: string | null;
  name: string;
  description: string;
  source: SourceLayer;
  review_status: ReviewStatus;
}

export interface ReadingStatusRow {
  user_id: string;
  work_id: string;
  status: ReadingLevel;
  starred: boolean;
  updated_at: string;
}

export interface Job {
  id: string;
  kind: string;
  status: JobStatus;
  attempts: number;
  last_error?: string | null;
  created_at: string;
  payload?: unknown;
  run_after?: string;
  locked_by?: string | null;
  locked_at?: string | null;
}

export interface PaperAspect {
  work_id: string;
  aspect: string;
  summary: string;
  bullets: string[] | unknown;
  source_text: string;
  page: number;
  model?: string | null;
  prompt_version?: string | null;
  created_at: string;
  updated_at: string;
}

export interface WorkCard {
  work: Work;
  primary_version?: Version | null;
  versions: Version[];
  authors: { author: { id: string; full_name: string }; position: number }[];
  projects: { id: string; name: string; description: string }[];
  claims: Claim[];
  methods: Method[];
  /** Fixed multi-aspect DNA layers. */
  aspects?: PaperAspect[];
  reading: ReadingStatusRow[];
  annotations_count: number;
  evidence: EvidenceSpan[];
  /** Assertion relations (excludes cites). Optional for older API. */
  relations?: RelationDetail[];
  /** Pipeline jobs for this work / versions. Optional for older API. */
  pipeline?: Job[];
}

export interface Annotation {
  id: string;
  work_id: string;
  user_id: string;
  kind: AnnotationKind;
  visibility: Visibility;
  body: string;
  anchor?: unknown;
  parent_id?: string | null;
  resolved?: boolean;
  version_id?: string | null;
  created_at: string;
}

export interface RelationDetail {
  relation: {
    id: string;
    type: RelationType;
    aspect?: string | null;
    scope?: string | null;
    explanation: string;
    confidence?: number | null;
    source: SourceLayer;
    review_status: ReviewStatus;
    created_at: string;
  };
  members: {
    relation_id: string;
    entity_kind: string;
    entity_id: string;
    role: string;
    anchor_work_id?: string | null;
  }[];
  evidence: {
    id: string;
    version_id: string;
    page: number;
    text: string;
    bbox?: unknown;
  }[];
  reviews: {
    id: string;
    user_id: string;
    verdict: 'agree' | 'disagree';
    comment: string;
  }[];
}

export interface MapNode {
  work_id: string;
  title: string;
  year?: number | null;
  readers: number;
  has_dispute: boolean;
  created_at: string;
}

export interface NeighborEntry {
  neighbor_work_id: string;
  score: number;
}

export interface MapEdge {
  relation_id: string;
  source_work_id: string;
  target_work_id: string;
  relation_type: RelationType;
  review_status: ReviewStatus;
  source_layer: SourceLayer;
  confidence?: number | null;
  explanation: string;
  review_count: number;
}

export interface MapResponse {
  nodes: MapNode[];
  neighbors: Record<string, Record<string, NeighborEntry[]>>;
  /** Assertion relations for near-LOD; similarity never draws edges. */
  edges?: MapEdge[];
}

export interface EgoNode {
  id: string;
  kind: string;
  label: string;
  work_id?: string | null;
  group_key?: string | null;
  group_count?: number | null;
  score?: number | null;
}

export interface EgoEdge {
  relation_id: string;
  source_id: string;
  target_id: string;
  relation_type: RelationType;
  review_status: ReviewStatus;
  source_layer: SourceLayer;
  confidence?: number | null;
  explanation: string;
  review_count: number;
  bundle_key?: string | null;
}

export interface EgoGroup {
  key: string;
  relation_type: RelationType;
  direction: string;
  count: number;
  member_work_ids: string[];
}

export interface EgoResponse {
  center: EgoNode;
  nodes: EgoNode[];
  edges: EgoEdge[];
  groups?: EgoGroup[];
}

export interface SavedView {
  id: string;
  name: string;
  weights: { citation_coupling?: number; method_lineage?: number; topic?: number };
  created_at: string;
}

export interface Project {
  id: string;
  name: string;
  description: string;
  created_at: string;
}

export interface ImportBatch {
  id: string;
  raw_input: string;
  parsed?: unknown;
  status: string;
  created_at: string;
}

export interface EvidenceSpan {
  id: string;
  relation_id?: string | null;
  claim_id?: string | null;
  extraction_field?: string | null;
  version_id: string;
  page: number;
  text: string;
  bbox?: unknown;
  created_at: string;
}

export interface ClaimJudgment {
  id: string;
  claim_id: string;
  user_id: string;
  verdict: ClaimVerdict | string;
  conditions: string;
  evidence_url?: string | null;
  created_at: string;
}

export interface ClaimWithEvidence {
  claim: Claim;
  evidence: EvidenceSpan[];
  judgments: ClaimJudgment[];
}
