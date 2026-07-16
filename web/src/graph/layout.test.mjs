/**
 * Pure layout helpers — run with: node src/graph/layout.test.mjs
 * (no vitest dependency for M3)
 */
import assert from 'node:assert/strict';

// Inline mirrors of layout.ts pure functions (keep in sync)
function springLength(score, lmin = 160, lmax = 720) {
  const s = Math.max(0, Math.min(1, score));
  const t = (1 - s) * (1 - s);
  return lmin + (lmax - lmin) * t;
}
function seedPosition(id) {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0;
  const x = ((h & 0xffff) / 0xffff) * 800 - 400;
  const y = (((h >>> 16) & 0xffff) / 0xffff) * 600 - 300;
  return { x, y };
}
function lodFromZoom(zoom, z1 = 0.6, z2 = 1.2) {
  if (zoom < z1) return 'far';
  if (zoom < z2) return 'mid';
  return 'near';
}
function semanticGroupOf(type) {
  const SEMANTIC = {
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
  return SEMANTIC[type] ?? 'other';
}
function statusRank(s) {
  if (s === 'disputed') return 3;
  if (s === 'confirmed') return 2;
  if (s === 'unreviewed') return 1;
  return 0;
}
function worseStatus(a, b) {
  return statusRank(b) > statusRank(a) ? b : a;
}
function bundleEdges(edges) {
  const map = new Map();
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
      if (existing.count > 1) existing.label = `${sg} ×${existing.count}`;
    }
  }
  const byPair = new Map();
  for (const b of map.values()) {
    const pair = [b.source_id, b.target_id].sort().join('|');
    if (!byPair.has(pair)) byPair.set(pair, []);
    byPair.get(pair).push(b);
  }
  const out = [];
  for (const list of byPair.values()) {
    list.sort((a, b) => b.count - a.count || statusRank(b.status) - statusRank(a.status));
    out.push(...list.slice(0, 3));
  }
  return out;
}
function readerBorderWidth(readers) {
  if (readers <= 0) return 1;
  if (readers === 1) return 2;
  if (readers === 2) return 3;
  return 4;
}
function combineNeighbors(neighbors, weights, topicEnabled) {
  const out = new Map();
  const dims = [
    { key: 'citation_coupling', w: weights.citation_coupling },
    { key: 'method_lineage', w: weights.method_lineage },
  ];
  if (topicEnabled && weights.topic > 0) dims.push({ key: 'topic', w: weights.topic });
  const wsum = dims.reduce((s, d) => s + d.w, 0) || 1;
  for (const { key, w } of dims) {
    const table = neighbors[key] ?? {};
    for (const [workId, list] of Object.entries(table)) {
      if (!out.has(workId)) out.set(workId, new Map());
      const m = out.get(workId);
      for (const n of list) {
        const prev = m.get(n.neighbor_work_id) ?? 0;
        m.set(n.neighbor_work_id, prev + (n.score * w) / wsum);
      }
    }
  }
  return out;
}

// --- tests ---
assert.equal(springLength(1), 160);
assert.equal(springLength(0), 720);
assert.ok(Math.abs(springLength(0.5) - (160 + 560 * 0.25)) < 1e-9);
assert.equal(lodFromZoom(0.3), 'far');
assert.equal(lodFromZoom(0.9), 'mid');
assert.equal(lodFromZoom(1.5), 'near');
assert.deepEqual(seedPosition('abc'), seedPosition('abc'));
assert.notDeepEqual(seedPosition('abc'), seedPosition('xyz'));
assert.equal(semanticGroupOf('improves_on'), 'method');
assert.equal(semanticGroupOf('fails_to_reproduce'), 'experiment');
assert.equal(readerBorderWidth(0), 1);
assert.equal(readerBorderWidth(5), 4);

const edges = [
  {
    relation_id: 'r1',
    source_id: 'a',
    target_id: 'b',
    relation_type: 'improves_on',
    review_status: 'confirmed',
    review_count: 1,
  },
  {
    relation_id: 'r2',
    source_id: 'b',
    target_id: 'a',
    relation_type: 'uses_method_from',
    review_status: 'unreviewed',
    review_count: 0,
  },
  {
    relation_id: 'r3',
    source_id: 'a',
    target_id: 'b',
    relation_type: 'supports_claim',
    review_status: 'disputed',
    review_count: 2,
  },
  {
    relation_id: 'r4',
    source_id: 'a',
    target_id: 'b',
    relation_type: 'compares_against',
    review_status: 'confirmed',
    review_count: 1,
  },
  {
    relation_id: 'r5',
    source_id: 'a',
    target_id: 'b',
    relation_type: 'reproduces',
    review_status: 'confirmed',
    review_count: 1,
  },
  {
    relation_id: 'r6',
    source_id: 'a',
    target_id: 'b',
    relation_type: 'prerequisite_for',
    review_status: 'unreviewed',
    review_count: 0,
  },
];
const bundles = bundleEdges(edges);
// method (2), experiment (2), argument (1), reading (1) → cap 3
assert.ok(bundles.length <= 3, `expected ≤3 bundles, got ${bundles.length}`);
const method = bundles.find((b) => b.semantic_group === 'method');
assert.ok(method);
assert.equal(method.count, 2);

const comb = combineNeighbors(
  {
    citation_coupling: {
      w1: [{ neighbor_work_id: 'w2', score: 1 }],
    },
    method_lineage: {
      w1: [{ neighbor_work_id: 'w2', score: 1 }],
    },
  },
  { citation_coupling: 0.5, method_lineage: 0.5, topic: 0 },
  false,
);
assert.ok(Math.abs(comb.get('w1').get('w2') - 1) < 1e-9);

console.log('layout.test.mjs: all passed');
