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

export interface User {
  id: string;
  email: string;
  name: string;
  role: UserRole;
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

export interface WorkCard {
  work: Work;
  primary_version?: Version | null;
  versions: Version[];
  authors: { author: { id: string; full_name: string }; position: number }[];
  projects: { id: string; name: string; description: string }[];
  claims: Claim[];
  methods: Method[];
  reading: ReadingStatusRow[];
  annotations_count: number;
  evidence: EvidenceSpan[];
}

export interface Annotation {
  id: string;
  work_id: string;
  user_id: string;
  kind: 'note' | 'conjecture' | 'question';
  visibility: 'private' | 'team';
  body: string;
  anchor?: unknown;
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

export interface MapResponse {
  nodes: MapNode[];
  neighbors: Record<string, Record<string, NeighborEntry[]>>;
}

export interface EgoResponse {
  center: { id: string; kind: string; label: string; work_id?: string | null };
  nodes: { id: string; kind: string; label: string; work_id?: string | null }[];
  edges: {
    relation_id: string;
    source_id: string;
    target_id: string;
    relation_type: RelationType;
    review_status: ReviewStatus;
    source_layer: SourceLayer;
    confidence?: number | null;
    explanation: string;
    review_count: number;
  }[];
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
  verdict: string;
  conditions: string;
  evidence_url?: string | null;
  created_at: string;
}

export interface ClaimWithEvidence {
  claim: Claim;
  evidence: EvidenceSpan[];
  judgments: ClaimJudgment[];
}
