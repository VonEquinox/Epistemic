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
  /** True when either endpoint ranks the other as its strongest neighbor. */
  primary: boolean;
  /** Best zero-based rank assigned by either endpoint. */
  bestRank: number;
  idealLength: number;
  elasticity: number;
}

export function layoutSpringLength(score: number, primary: boolean): number {
  const similarity = Math.max(0, Math.min(1, score));
  const falloff = (1 - similarity) * (1 - similarity);
  // Primary links form local geometry; secondary links only keep clusters coherent.
  return primary ? 70 + 260 * falloff : 360 + 220 * falloff;
}

export function layoutSpringElasticity(primary: boolean, bestRank: number): number {
  if (primary) return 6;
  return 0.03 / Math.max(1, bestRank);
}

/** Convert directed top-K lists into canonical undirected layout springs. */
export function buildLayoutSprings(
  scores: Map<string, Map<string, number>>,
): LayoutSpring[] {
  const pairs = new Map<
    string,
    Omit<LayoutSpring, 'idealLength' | 'elasticity'>
  >();

  for (const [sourceId, neighbors] of scores) {
    const ranked = [...neighbors]
      .filter(
        ([targetId, score]) => targetId !== sourceId && Number.isFinite(score),
      )
      .sort((left, right) => right[1] - left[1]);
    ranked.forEach(([targetId, score], rank) => {
      const [source, target] = [sourceId, targetId].sort();
      const key = `${source}|${target}`;
      const existing = pairs.get(key);
      if (!existing) {
        pairs.set(key, {
          key,
          sourceId: source,
          targetId: target,
          score,
          primary: rank === 0,
          bestRank: rank,
        });
        return;
      }
      existing.score = Math.max(existing.score, score);
      existing.primary ||= rank === 0;
      existing.bestRank = Math.min(existing.bestRank, rank);
    });
  }

  return [...pairs.values()].map((spring) => ({
    ...spring,
    idealLength: layoutSpringLength(spring.score, spring.primary),
    elasticity: layoutSpringElasticity(spring.primary, spring.bestRank),
  }));
}

export interface GraphPosition {
  x: number;
  y: number;
}

export interface CollisionBounds {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export interface NearestNeighborRefinementOptions {
  iterations: number;
  margin: number;
  step: number;
  maxMove: number;
  minSeparation: number;
  collisionPadding: number;
  collisionRepulsion: number;
  anchorStrength: number;
}

const DEFAULT_REFINEMENT: NearestNeighborRefinementOptions = {
  iterations: 120,
  margin: 8,
  step: 0.35,
  maxMove: 12,
  minSeparation: 45,
  collisionPadding: 8,
  collisionRepulsion: 0.12,
  anchorStrength: 0.004,
};

/**
 * Ordinal post-pass for a global layout. It pulls each node toward its
 * strongest neighbor only when another node is geometrically closer, while
 * collision and anchor forces preserve readability and the overall map shape.
 */
export function refineNearestNeighborPositions(
  positions: Map<string, GraphPosition>,
  scores: Map<string, Map<string, number>>,
  options: Partial<NearestNeighborRefinementOptions> = {},
  collisionBounds?: Map<string, CollisionBounds>,
): Map<string, GraphPosition> {
  const config = { ...DEFAULT_REFINEMENT, ...options };
  const ids = [...positions.keys()];
  const refined = new Map(
    [...positions].map(([id, position]) => [id, { ...position }]),
  );
  const anchors = new Map(
    [...positions].map(([id, position]) => [id, { ...position }]),
  );
  if (ids.length < 3) return refined;

  const strongest = new Map<string, string>();
  for (const [sourceId, neighbors] of scores) {
    if (!refined.has(sourceId)) continue;
    const target = [...neighbors]
      .filter(
        ([targetId, score]) => refined.has(targetId) && Number.isFinite(score),
      )
      .sort((left, right) => right[1] - left[1])[0];
    if (target) strongest.set(sourceId, target[0]);
  }

  const unit = (dx: number, dy: number, key: string) => {
    const distance = Math.hypot(dx, dy);
    if (distance > 1e-6) return { x: dx / distance, y: dy / distance, distance };
    const fallback = seedPosition(key);
    const fallbackLength = Math.hypot(fallback.x, fallback.y) || 1;
    return {
      x: fallback.x / fallbackLength,
      y: fallback.y / fallbackLength,
      distance: 0,
    };
  };

  for (let iteration = 0; iteration < config.iterations; iteration += 1) {
    const deltas = new Map(ids.map((id) => [id, { x: 0, y: 0 }]));
    const distance = (leftId: string, rightId: string) => {
      const left = refined.get(leftId)!;
      const right = refined.get(rightId)!;
      return Math.hypot(left.x - right.x, left.y - right.y);
    };

    for (const sourceId of ids) {
      const targetId = strongest.get(sourceId);
      if (!targetId) continue;
      let rivalId: string | null = null;
      let rivalDistance = Number.POSITIVE_INFINITY;
      for (const candidateId of ids) {
        if (candidateId === sourceId || candidateId === targetId) continue;
        const candidateDistance = distance(sourceId, candidateId);
        if (candidateDistance < rivalDistance) {
          rivalId = candidateId;
          rivalDistance = candidateDistance;
        }
      }
      if (!rivalId) continue;

      const targetDistance = distance(sourceId, targetId);
      const violation = targetDistance + config.margin - rivalDistance;
      if (violation <= 0) continue;
      const amount = Math.min(config.maxMove, violation * config.step);
      const source = refined.get(sourceId)!;
      const target = refined.get(targetId)!;
      const rival = refined.get(rivalId)!;
      const towardTarget = unit(
        target.x - source.x,
        target.y - source.y,
        `${sourceId}|${targetId}`,
      );
      deltas.get(sourceId)!.x += towardTarget.x * amount * 0.45;
      deltas.get(sourceId)!.y += towardTarget.y * amount * 0.45;
      deltas.get(targetId)!.x -= towardTarget.x * amount * 0.45;
      deltas.get(targetId)!.y -= towardTarget.y * amount * 0.45;

      const awayFromRival = unit(
        source.x - rival.x,
        source.y - rival.y,
        `${sourceId}|${rivalId}`,
      );
      deltas.get(sourceId)!.x += awayFromRival.x * amount * 0.2;
      deltas.get(sourceId)!.y += awayFromRival.y * amount * 0.2;
      deltas.get(rivalId)!.x -= awayFromRival.x * amount * 0.2;
      deltas.get(rivalId)!.y -= awayFromRival.y * amount * 0.2;
    }

    for (let leftIndex = 0; leftIndex < ids.length; leftIndex += 1) {
      for (let rightIndex = leftIndex + 1; rightIndex < ids.length; rightIndex += 1) {
        const leftId = ids[leftIndex];
        const rightId = ids[rightIndex];
        const left = refined.get(leftId)!;
        const right = refined.get(rightId)!;
        const leftBounds = collisionBounds?.get(leftId);
        const rightBounds = collisionBounds?.get(rightId);
        if (leftBounds && rightBounds) {
          const overlapX =
            Math.min(
              left.x + leftBounds.right,
              right.x + rightBounds.right,
            ) -
              Math.max(
                left.x + leftBounds.left,
                right.x + rightBounds.left,
              ) +
            config.collisionPadding;
          const overlapY =
            Math.min(
              left.y + leftBounds.bottom,
              right.y + rightBounds.bottom,
            ) -
              Math.max(
                left.y + leftBounds.top,
                right.y + rightBounds.top,
              ) +
            config.collisionPadding;
          if (overlapX <= 0 || overlapY <= 0) continue;
          if (overlapX < overlapY) {
            const direction = left.x <= right.x ? -1 : 1;
            const amount = overlapX * config.collisionRepulsion;
            deltas.get(leftId)!.x += direction * amount;
            deltas.get(rightId)!.x -= direction * amount;
          } else {
            const direction = left.y <= right.y ? -1 : 1;
            const amount = overlapY * config.collisionRepulsion;
            deltas.get(leftId)!.y += direction * amount;
            deltas.get(rightId)!.y -= direction * amount;
          }
          continue;
        }

        const direction = unit(
          left.x - right.x,
          left.y - right.y,
          `${leftId}|${rightId}`,
        );
        if (direction.distance >= config.minSeparation) continue;
        const amount =
          (config.minSeparation - direction.distance) * config.collisionRepulsion;
        deltas.get(leftId)!.x += direction.x * amount;
        deltas.get(leftId)!.y += direction.y * amount;
        deltas.get(rightId)!.x -= direction.x * amount;
        deltas.get(rightId)!.y -= direction.y * amount;
      }
    }

    for (const id of ids) {
      const position = refined.get(id)!;
      const anchor = anchors.get(id)!;
      const delta = deltas.get(id)!;
      delta.x += (anchor.x - position.x) * config.anchorStrength;
      delta.y += (anchor.y - position.y) * config.anchorStrength;
      const magnitude = Math.hypot(delta.x, delta.y);
      const scale = magnitude > config.maxMove ? config.maxMove / magnitude : 1;
      position.x += delta.x * scale;
      position.y += delta.y * scale;
    }
  }

  return refined;
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
  refineNearestNeighborPositions,
  bundleEdges,
  semanticGroupOf,
  readerBorderWidth,
};
