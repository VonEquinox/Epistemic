import type { EgoEdge, MapNode, NeighborEntry } from '../api/types';

export interface Weights {
  citation_coupling: number;
  method_lineage: number;
  topic: number;
}

/** Combine multi-dimension neighbor tables into score map: workId -> neighborId -> score */
export function combineNeighbors(
  neighbors: Record<string, Record<string, NeighborEntry[]>>,
  weights: Weights,
  topicEnabled: boolean,
): Map<string, Map<string, number>> {
  const out = new Map<string, Map<string, number>>();
  const dims: { key: string; w: number }[] = [
    { key: 'citation_coupling', w: weights.citation_coupling },
    { key: 'method_lineage', w: weights.method_lineage },
  ];
  if (topicEnabled && weights.topic > 0) {
    dims.push({ key: 'topic', w: weights.topic });
  }
  const wsum = dims.reduce((s, d) => s + d.w, 0) || 1;

  for (const { key, w } of dims) {
    const table = neighbors[key] ?? {};
    for (const [workId, list] of Object.entries(table)) {
      if (!out.has(workId)) out.set(workId, new Map());
      const m = out.get(workId)!;
      for (const n of list) {
        const prev = m.get(n.neighbor_work_id) ?? 0;
        m.set(n.neighbor_work_id, prev + (n.score * w) / wsum);
      }
    }
  }
  return out;
}

/** Spring length from combined score. Lmin=40, Lmax=280 */
export function springLength(score: number, lmin = 40, lmax = 280): number {
  const s = Math.max(0, Math.min(1, score));
  return lmin + (lmax - lmin) * (1 - s);
}

/** Deterministic seed position from work id hash */
export function seedPosition(id: string): { x: number; y: number } {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0;
  const x = ((h & 0xffff) / 0xffff) * 800 - 400;
  const y = (((h >>> 16) & 0xffff) / 0xffff) * 600 - 300;
  return { x, y };
}

export function unconnectedNodes(
  nodes: MapNode[],
  combined: Map<string, Map<string, number>>,
): Set<string> {
  const s = new Set<string>();
  for (const n of nodes) {
    const m = combined.get(n.work_id);
    if (!m || m.size === 0) s.add(n.work_id);
  }
  return s;
}

export function lodFromZoom(zoom: number, z1 = 0.6, z2 = 1.2): 'far' | 'mid' | 'near' {
  if (zoom < z1) return 'far';
  if (zoom < z2) return 'mid';
  return 'near';
}

export interface EdgeBundle {
  key: string;
  source_id: string;
  target_id: string;
  semantic_group: string;
  count: number;
  /** Representative status: disputed > confirmed > unreviewed */
  status: string;
  review_count: number;
  relation_ids: string[];
  label: string;
}

const SEMANTIC: Record<string, string> = {
  uses_method_from: 'method',
  improves_on: 'method',
  alternative_to: 'method',
  uses_dataset_from: 'experiment',
  compares_against: 'experiment',
  reproduces: 'experiment',
  fails_to_reproduce: 'experiment',
  supports_claim: 'argument',
  contradicts_claim: 'argument',
  prerequisite_for: 'reading',
  cites: 'meta',
  version_of: 'meta',
};

export function semanticGroupOf(type: string): string {
  return SEMANTIC[type] ?? 'other';
}

/**
 * Bundle edges by (pair × semantic group). Max 3 visual edges per paper pair
 * (DEV.md §6.4) — keep highest-priority groups if more.
 */
export function bundleEdges(edges: EgoEdge[]): EdgeBundle[] {
  const map = new Map<string, EdgeBundle>();
  for (const e of edges) {
    const sg = e.bundle_key?.split('|')[2] ?? semanticGroupOf(e.relation_type);
    const pair = [e.source_id, e.target_id].sort().join('|');
    const key = `${pair}|${sg}`;
    const existing = map.get(key);
    if (!existing) {
      map.set(key, {
        key,
        source_id: e.source_id,
        target_id: e.target_id,
        semantic_group: sg,
        count: 1,
        status: e.review_status,
        review_count: e.review_count,
        relation_ids: [e.relation_id],
        label: e.relation_type.replace(/_/g, ' '),
      });
    } else {
      existing.count += 1;
      existing.relation_ids.push(e.relation_id);
      existing.review_count = Math.max(existing.review_count, e.review_count);
      existing.status = worseStatus(existing.status, e.review_status);
      if (existing.count > 1) {
        existing.label = `${sg} ×${existing.count}`;
      }
    }
  }

  // Cap 3 bundles per unordered pair
  const byPair = new Map<string, EdgeBundle[]>();
  for (const b of map.values()) {
    const pair = [b.source_id, b.target_id].sort().join('|');
    if (!byPair.has(pair)) byPair.set(pair, []);
    byPair.get(pair)!.push(b);
  }
  const out: EdgeBundle[] = [];
  for (const list of byPair.values()) {
    list.sort((a, b) => b.count - a.count || statusRank(b.status) - statusRank(a.status));
    out.push(...list.slice(0, 3));
  }
  return out;
}

function statusRank(s: string): number {
  if (s === 'disputed') return 3;
  if (s === 'confirmed') return 2;
  if (s === 'unreviewed') return 1;
  return 0;
}

function worseStatus(a: string, b: string): string {
  return statusRank(b) > statusRank(a) ? b : a;
}

/** Node border width from reader count (team overlay). */
export function readerBorderWidth(readers: number): number {
  if (readers <= 0) return 1;
  if (readers === 1) return 2;
  if (readers === 2) return 3;
  return 4;
}

/** Pure function unit-test targets */
export const __test = {
  springLength,
  seedPosition,
  lodFromZoom,
  combineNeighbors,
  bundleEdges,
  semanticGroupOf,
  readerBorderWidth,
};
