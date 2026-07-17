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
function topNeighborMap(scores, topK = 4, minScore = 0) {
  const out = new Map();
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
function layoutSpringLength(score, primary) {
  const similarity = Math.max(0, Math.min(1, score));
  const falloff = (1 - similarity) * (1 - similarity);
  return primary ? 70 + 260 * falloff : 360 + 220 * falloff;
}
function layoutSpringElasticity(primary, bestRank) {
  if (primary) return 6;
  return 0.03 / Math.max(1, bestRank);
}
function buildLayoutSprings(scores) {
  const pairs = new Map();
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
      } else {
        existing.score = Math.max(existing.score, score);
        existing.primary ||= rank === 0;
        existing.bestRank = Math.min(existing.bestRank, rank);
      }
    });
  }
  return [...pairs.values()].map((spring) => ({
    ...spring,
    idealLength: layoutSpringLength(spring.score, spring.primary),
    elasticity: layoutSpringElasticity(spring.primary, spring.bestRank),
  }));
}
function refineNearestNeighborPositions(
  positions,
  scores,
  options = {},
  collisionBounds,
) {
  const config = {
    iterations: 120,
    margin: 8,
    step: 0.35,
    maxMove: 12,
    minSeparation: 45,
    collisionPadding: 8,
    collisionRepulsion: 0.12,
    anchorStrength: 0.004,
    ...options,
  };
  const ids = [...positions.keys()];
  const refined = new Map([...positions].map(([id, value]) => [id, { ...value }]));
  const anchors = new Map([...positions].map(([id, value]) => [id, { ...value }]));
  const strongest = new Map();
  for (const [sourceId, neighbors] of scores) {
    const target = [...neighbors]
      .filter(([targetId, score]) => refined.has(targetId) && Number.isFinite(score))
      .sort((left, right) => right[1] - left[1])[0];
    if (target && refined.has(sourceId)) strongest.set(sourceId, target[0]);
  }
  const unit = (dx, dy, key) => {
    const distance = Math.hypot(dx, dy);
    if (distance > 1e-6) return { x: dx / distance, y: dy / distance, distance };
    const fallback = seedPosition(key);
    const fallbackLength = Math.hypot(fallback.x, fallback.y) || 1;
    return { x: fallback.x / fallbackLength, y: fallback.y / fallbackLength, distance: 0 };
  };
  for (let iteration = 0; iteration < config.iterations; iteration += 1) {
    const deltas = new Map(ids.map((id) => [id, { x: 0, y: 0 }]));
    const distance = (leftId, rightId) => {
      const left = refined.get(leftId);
      const right = refined.get(rightId);
      return Math.hypot(left.x - right.x, left.y - right.y);
    };
    for (const sourceId of ids) {
      const targetId = strongest.get(sourceId);
      if (!targetId) continue;
      let rivalId = null;
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
      const source = refined.get(sourceId);
      const target = refined.get(targetId);
      const rival = refined.get(rivalId);
      const towardTarget = unit(target.x - source.x, target.y - source.y, `${sourceId}|${targetId}`);
      deltas.get(sourceId).x += towardTarget.x * amount * 0.45;
      deltas.get(sourceId).y += towardTarget.y * amount * 0.45;
      deltas.get(targetId).x -= towardTarget.x * amount * 0.45;
      deltas.get(targetId).y -= towardTarget.y * amount * 0.45;
      const awayFromRival = unit(source.x - rival.x, source.y - rival.y, `${sourceId}|${rivalId}`);
      deltas.get(sourceId).x += awayFromRival.x * amount * 0.2;
      deltas.get(sourceId).y += awayFromRival.y * amount * 0.2;
      deltas.get(rivalId).x -= awayFromRival.x * amount * 0.2;
      deltas.get(rivalId).y -= awayFromRival.y * amount * 0.2;
    }
    for (let leftIndex = 0; leftIndex < ids.length; leftIndex += 1) {
      for (let rightIndex = leftIndex + 1; rightIndex < ids.length; rightIndex += 1) {
        const leftId = ids[leftIndex];
        const rightId = ids[rightIndex];
        const left = refined.get(leftId);
        const right = refined.get(rightId);
        const leftBounds = collisionBounds?.get(leftId);
        const rightBounds = collisionBounds?.get(rightId);
        if (leftBounds && rightBounds) {
          const overlapX =
            Math.min(left.x + leftBounds.right, right.x + rightBounds.right) -
              Math.max(left.x + leftBounds.left, right.x + rightBounds.left) +
            config.collisionPadding;
          const overlapY =
            Math.min(left.y + leftBounds.bottom, right.y + rightBounds.bottom) -
              Math.max(left.y + leftBounds.top, right.y + rightBounds.top) +
            config.collisionPadding;
          if (overlapX <= 0 || overlapY <= 0) continue;
          if (overlapX < overlapY) {
            const direction = left.x <= right.x ? -1 : 1;
            const amount = overlapX * config.collisionRepulsion;
            deltas.get(leftId).x += direction * amount;
            deltas.get(rightId).x -= direction * amount;
          } else {
            const direction = left.y <= right.y ? -1 : 1;
            const amount = overlapY * config.collisionRepulsion;
            deltas.get(leftId).y += direction * amount;
            deltas.get(rightId).y -= direction * amount;
          }
          continue;
        }
        const direction = unit(left.x - right.x, left.y - right.y, `${leftId}|${rightId}`);
        if (direction.distance >= config.minSeparation) continue;
        const amount = (config.minSeparation - direction.distance) * config.collisionRepulsion;
        deltas.get(leftId).x += direction.x * amount;
        deltas.get(leftId).y += direction.y * amount;
        deltas.get(rightId).x -= direction.x * amount;
        deltas.get(rightId).y -= direction.y * amount;
      }
    }
    for (const id of ids) {
      const position = refined.get(id);
      const anchor = anchors.get(id);
      const delta = deltas.get(id);
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
      if (existing.count > 1) existing.label = `${sg} ×${existing.count}`;
    }
  }
  const byPair = new Map();
  for (const b of map.values()) {
    const pair = b.symmetric
      ? `s|${[b.source_id, b.target_id].sort().join('|')}`
      : `d|${b.source_id}|${b.target_id}`;
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
// Opposite directed method edges must remain separate bundles.
const forwardMethod = bundles.find(
  (b) => b.semantic_group === 'method' && b.source_id === 'a' && b.target_id === 'b',
);
const reverseMethod = bundles.find(
  (b) => b.semantic_group === 'method' && b.source_id === 'b' && b.target_id === 'a',
);
assert.ok(forwardMethod);
assert.ok(reverseMethod);
assert.equal(forwardMethod.count, 1);
assert.equal(reverseMethod.count, 1);

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

const trimmed = topNeighborMap(
  new Map([
    ['a', new Map([['b', 0.9], ['c', 0.7], ['d', 0.2]])],
  ]),
  2,
  0.3,
);
assert.deepEqual([...trimmed.get('a').keys()], ['b', 'c']);

const springs = buildLayoutSprings(
  new Map([
    ['a', new Map([['b', 0.9], ['c', 0.7]])],
    ['b', new Map([['a', 0.85], ['c', 0.6]])],
  ]),
);
const primarySpring = springs.find((spring) => spring.key === 'a|b');
const secondarySpring = springs.find((spring) => spring.key === 'a|c');
assert.equal(springs.filter((spring) => spring.key === 'a|b').length, 1);
assert.equal(primarySpring.primary, true);
assert.equal(primarySpring.score, 0.9);
assert.ok(primarySpring.idealLength < secondarySpring.idealLength);
assert.ok(primarySpring.elasticity > secondarySpring.elasticity);

const initialPositions = new Map([
  ['a', { x: 0, y: 0 }],
  ['b', { x: 120, y: 0 }],
  ['c', { x: 20, y: 0 }],
  ['d', { x: 0, y: 100 }],
]);
const refinedPositions = refineNearestNeighborPositions(
  initialPositions,
  new Map([['a', new Map([['b', 0.9], ['c', 0.5]])]]),
  { iterations: 240, margin: 1, minSeparation: 0, anchorStrength: 0 },
);
const distance = (left, right) =>
  Math.hypot(left.x - right.x, left.y - right.y);
assert.ok(
  distance(refinedPositions.get('a'), refinedPositions.get('b')) <
    distance(refinedPositions.get('a'), refinedPositions.get('c')),
);
assert.deepEqual(initialPositions.get('a'), { x: 0, y: 0 });

const overlappingPositions = new Map([
  ['a', { x: 0, y: 0 }],
  ['b', { x: 15, y: 0 }],
  ['c', { x: 200, y: 0 }],
]);
const collisionBoxes = new Map([
  ['a', { left: -35, right: 35, top: -10, bottom: 22 }],
  ['b', { left: -35, right: 35, top: -10, bottom: 22 }],
  ['c', { left: -35, right: 35, top: -10, bottom: 22 }],
]);
const separatedPositions = refineNearestNeighborPositions(
  overlappingPositions,
  new Map(),
  { iterations: 160, anchorStrength: 0 },
  collisionBoxes,
);
assert.ok(
  Math.abs(separatedPositions.get('a').x - separatedPositions.get('b').x) >= 78 ||
    Math.abs(separatedPositions.get('a').y - separatedPositions.get('b').y) >= 39.9,
);

console.log('layout.test.mjs: all passed');
