import type { MapNode, NeighborEntry } from '../api/types';

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
  const x = (h & 0xffff) / 0xffff * 800 - 400;
  const y = ((h >>> 16) & 0xffff) / 0xffff * 600 - 300;
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

/** Pure function unit-test targets */
export const __test = { springLength, seedPosition, lodFromZoom, combineNeighbors };
