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

/**
 * Single aspect dimension → score map (for layout springs + visible sim edges).
 * Optionally caps each work's outgoing list to topK and drops scores below minScore.
 */
export function aspectNeighborMap(
  neighbors: Record<string, Record<string, NeighborEntry[]>>,
  dimension: string,
  topK = 8,
  minScore = 0,
): Map<string, Map<string, number>> {
  const out = new Map<string, Map<string, number>>();
  const table = neighbors[dimension] ?? {};
  for (const [workId, list] of Object.entries(table)) {
    const sorted = [...list]
      .filter((n) => n.score >= minScore)
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);
    if (sorted.length === 0) continue;
    const m = new Map<string, number>();
    for (const n of sorted) {
      m.set(n.neighbor_work_id, n.score);
    }
    out.set(workId, m);
  }
  return out;
}

/** Keep only the strongest outgoing similarities for each node. */
export function topNeighborMap(
  scores: Map<string, Map<string, number>>,
  topK = 4,
  minScore = 0,
): Map<string, Map<string, number>> {
  const out = new Map<string, Map<string, number>>();
  for (const [workId, neighbors] of scores) {
    const strongest = [...neighbors]
      .filter(
        ([neighborId, score]) =>
          neighborId !== workId && Number.isFinite(score) && score >= minScore,
      )
      .sort((left, right) => right[1] - left[1])
      .slice(0, topK);
    if (strongest.length > 0) out.set(workId, new Map(strongest));
  }
  return out;
}

export interface LayoutSpring {
  key: string;
  sourceId: string;
  targetId: string;
  score: number;
  idealLength: number;
  elasticity: number;
}

function normalizedSpringWeight(score: number): number {
  // Most aspect similarities live around 0.35-0.90. Remapping that band makes
  // medium and weak relations decay much faster while preserving strong links.
  return Math.max(0, Math.min(1, (score - 0.35) / 0.55));
}

/** Stronger similarity means a shorter target distance. */
export function layoutSpringLength(score: number): number {
  const similarity = Math.max(0, Math.min(1, score));
  return 90 + 430 * Math.pow(1 - similarity, 1.6);
}

/** Keep strong links strong, but make attraction decay faster below them. */
export function layoutSpringElasticity(score: number): number {
  const weight = normalizedSpringWeight(score);
  return 0.08 + 5.5 * Math.pow(weight, 4);
}

/** Convert directed top-K lists into weighted undirected physical springs. */
export function buildLayoutSprings(
  scores: Map<string, Map<string, number>>,
): LayoutSpring[] {
  const pairs = new Map<
    string,
    Omit<LayoutSpring, 'idealLength' | 'elasticity'>
  >();

  for (const [sourceId, neighbors] of scores) {
    for (const [targetId, score] of neighbors) {
      if (targetId === sourceId || !Number.isFinite(score)) continue;
      const [source, target] = [sourceId, targetId].sort();
      const key = `${source}|${target}`;
      const existing = pairs.get(key);
      if (!existing) {
        pairs.set(key, {
          key,
          sourceId: source,
          targetId: target,
          score,
        });
      } else {
        existing.score = Math.max(existing.score, score);
      }
    }
  }

  return [...pairs.values()].map((spring) => ({
    ...spring,
    idealLength: layoutSpringLength(spring.score),
    elasticity: layoutSpringElasticity(spring.score),
  }));
}

export interface ForceSimulationConfig {
  attractionStrength: number;
  repulsionStrength: number;
  attractionDecay: number;
  repulsionDecay: number;
}

export const DEFAULT_FORCE_TUNING: ForceSimulationConfig = {
  attractionStrength: 2,
  repulsionStrength: 0.5,
  attractionDecay: 2,
  repulsionDecay: 1,
};

export interface SimulationPosition {
  x: number;
  y: number;
}

/** Explicit weighted force simulation used by the live tuning sliders. */
export function simulateWeightedForces(
  positions: Map<string, SimulationPosition>,
  springs: LayoutSpring[],
  tuning: ForceSimulationConfig,
  iterations = 140,
): Map<string, SimulationPosition> {
  const ids = [...positions.keys()];
  const index = new Map(ids.map((id, position) => [id, position]));
  const x = new Float64Array(ids.length);
  const y = new Float64Array(ids.length);
  const vx = new Float64Array(ids.length);
  const vy = new Float64Array(ids.length);
  ids.forEach((id, position) => {
    const point = positions.get(id)!;
    x[position] = point.x;
    y[position] = point.y;
  });
  const indexedSprings = springs.flatMap((spring) => {
    const source = index.get(spring.sourceId);
    const target = index.get(spring.targetId);
    return source === undefined || target === undefined
      ? []
      : [{ spring, source, target }];
  });

  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const fx = new Float64Array(ids.length);
    const fy = new Float64Array(ids.length);
    const cooling = 0.1 + 0.9 * (1 - iteration / iterations);

    for (const { spring, source, target } of indexedSprings) {
      const dx = x[target] - x[source];
      const dy = y[target] - y[source];
      const distance = Math.hypot(dx, dy) || 0.001;
      const weight = Math.pow(
        normalizedSpringWeight(spring.score),
        tuning.attractionDecay,
      );
      const magnitude =
        tuning.attractionStrength * weight * (distance - spring.idealLength) * 0.04;
      const forceX = (dx / distance) * magnitude;
      const forceY = (dy / distance) * magnitude;
      fx[source] += forceX;
      fy[source] += forceY;
      fx[target] -= forceX;
      fy[target] -= forceY;
    }

    for (let left = 0; left < ids.length; left += 1) {
      for (let right = left + 1; right < ids.length; right += 1) {
        let dx = x[left] - x[right];
        let dy = y[left] - y[right];
        let distance = Math.hypot(dx, dy);
        if (distance < 0.001) {
          const fallback = seedPosition(`${ids[left]}|${ids[right]}`);
          dx = fallback.x || 1;
          dy = fallback.y;
          distance = Math.hypot(dx, dy);
        }
        const effectiveDistance = Math.max(18, distance);
        const magnitude =
          (tuning.repulsionStrength * 2.5) /
          Math.pow(effectiveDistance / 100, tuning.repulsionDecay);
        const forceX = (dx / distance) * magnitude;
        const forceY = (dy / distance) * magnitude;
        fx[left] += forceX;
        fy[left] += forceY;
        fx[right] -= forceX;
        fy[right] -= forceY;
      }
    }

    for (let position = 0; position < ids.length; position += 1) {
      // Weak centering prevents the entire system from drifting off-canvas.
      fx[position] -= x[position] * 0.0007;
      fy[position] -= y[position] * 0.0007;
      vx[position] = (vx[position] + fx[position] * cooling) * 0.82;
      vy[position] = (vy[position] + fy[position] * cooling) * 0.82;
      const speed = Math.hypot(vx[position], vy[position]);
      const scale = speed > 12 ? 12 / speed : 1;
      x[position] += vx[position] * scale;
      y[position] += vy[position] * scale;
    }
  }

  return new Map(
    ids.map((id, position) => [id, { x: x[position], y: y[position] }]),
  );
}

/**
 * Spring length from combined score.
 * High similarity → shorter spring; weak pairs stay far apart so the map breathes.
 * Defaults tuned for ~80-node aspect maps (Lmin=160, Lmax=720).
 */
export function springLength(score: number, lmin = 160, lmax = 720): number {
  const s = Math.max(0, Math.min(1, score));
  // Square falloff: mid scores still get fairly long springs.
  const t = (1 - s) * (1 - s);
  return lmin + (lmax - lmin) * t;
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

/** Keep labels roughly screen-sized, then shrink them slightly at high zoom. */
export function labelFontSizeForZoom(zoom: number, selected = false): number {
  const safeZoom = Math.max(0.45, Math.min(3, zoom));
  const base = selected ? 9 : 8;
  return Math.max(2.2, Math.min(14, base / Math.pow(safeZoom, 1.15)));
}

export interface EdgeBundle {
  key: string;
  source_id: string;
  target_id: string;
  semantic_group: string;
  symmetric: boolean;
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
    const symmetric = e.relation_type === 'alternative_to';
    const pair = symmetric
      ? [e.source_id, e.target_id].sort().join('|')
      : `${e.source_id}|${e.target_id}`;
    const key = `${symmetric ? 's' : 'd'}|${pair}|${sg}`;
    const existing = map.get(key);
    if (!existing) {
      map.set(key, {
        key,
        source_id: e.source_id,
        target_id: e.target_id,
        semantic_group: sg,
        symmetric,
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
    const pair = b.symmetric
      ? `s|${[b.source_id, b.target_id].sort().join('|')}`
      : `d|${b.source_id}|${b.target_id}`;
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
  labelFontSizeForZoom,
  combineNeighbors,
  aspectNeighborMap,
  topNeighborMap,
  buildLayoutSprings,
  layoutSpringLength,
  layoutSpringElasticity,
  simulateWeightedForces,
  bundleEdges,
  semanticGroupOf,
  readerBorderWidth,
};
