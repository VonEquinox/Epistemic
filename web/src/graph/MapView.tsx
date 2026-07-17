import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
// @ts-expect-error no types
import fcose from 'cytoscape-fcose';
import type { MapEdge, MapResponse } from '../api/types';
import { useUiStore } from '../stores/ui';
import { COLORS, GRAPH_FONT, cyStylesheet } from './styles';
import {
  aspectNeighborMap,
  buildLayoutSprings,
  combineNeighbors,
  refineNearestNeighborPositions,
  seedPosition,
  topNeighborMap,
  unconnectedNodes,
  lodFromZoom,
  readerBorderWidth,
  bundleEdges,
} from './layout';
import {
  ASPECT_EDGE_BUILD_MIN,
  ASPECT_EDGE_TOP_K,
  ASPECT_LAYOUT_MIN_SCORE,
  ASPECT_LAYOUT_TOP_K,
  aspectByKey,
} from './aspects';

cytoscape.use(fcose);

interface Props {
  data: MapResponse;
  onSelect: (workId: string) => void;
  onOpenEgo: (workId: string) => void;
  onSelectEdge?: (relationId: string) => void;
  /** Show unreviewed assertion candidates (default false in aspect mode). */
  showCandidates?: boolean;
}

const HIGH_RISK = new Set(['fails_to_reproduce', 'contradicts_claim']);

/** Near-LOD assertion edges; hide high-risk until confirmed (PRD §6.4). */
function visibleMapEdges(edges: MapEdge[] | undefined, showCandidates: boolean): MapEdge[] {
  if (!edges) return [];
  return edges.filter((e) => {
    if (e.relation_type === 'cites') return false;
    if (e.review_status === 'rejected') return false;
    if (HIGH_RISK.has(e.relation_type) && e.review_status !== 'confirmed') return false;
    if (!showCandidates && e.review_status === 'unreviewed') return false;
    return true;
  });
}

function aspectDim(activeAspect: string): string {
  return aspectByKey(activeAspect)?.dimension ?? `aspect_${activeAspect}`;
}

/** Dense map for drawn edges (filtered live by minSimScore). */
function buildDisplayScoreMap(
  data: MapResponse,
  activeAspect: string | null,
  weights: { citation_coupling: number; method_lineage: number; topic: number },
  topicEnabled: boolean,
): Map<string, Map<string, number>> {
  if (activeAspect) {
    return aspectNeighborMap(
      data.neighbors,
      aspectDim(activeAspect),
      ASPECT_EDGE_TOP_K,
      ASPECT_EDGE_BUILD_MIN,
    );
  }
  return combineNeighbors(data.neighbors, weights, topicEnabled);
}

/** Sparse strong springs only — keeps the graph from collapsing into a ball. */
function buildLayoutScoreMap(
  data: MapResponse,
  activeAspect: string | null,
  weights: { citation_coupling: number; method_lineage: number; topic: number },
  topicEnabled: boolean,
): Map<string, Map<string, number>> {
  if (activeAspect) {
    return aspectNeighborMap(
      data.neighbors,
      aspectDim(activeAspect),
      ASPECT_LAYOUT_TOP_K,
      ASPECT_LAYOUT_MIN_SCORE,
    );
  }
  return topNeighborMap(
    combineNeighbors(data.neighbors, weights, topicEnabled),
    ASPECT_LAYOUT_TOP_K,
    0.05,
  );
}

const FCOSE_SPREAD = {
  nodeRepulsion: () => 52000,
  nodeSeparation: 100,
  gravity: 0.035,
  gravityRange: 5.5,
  numIter: 5000,
  packComponents: true,
  idealDefault: 420,
};

function refineGlobalLayout(
  cy: Core,
  scores: Map<string, Map<string, number>>,
) {
  const movableNodes = cy.nodes().filter((node) => !node.locked());
  const positions = new Map<string, { x: number; y: number }>();
  movableNodes.forEach((element) => {
    const node = element as cytoscape.NodeSingular;
    positions.set(node.id(), { ...node.position() });
  });
  const refined = refineNearestNeighborPositions(positions, scores);
  cy.batch(() => {
    movableNodes.forEach((element) => {
      const node = element as cytoscape.NodeSingular;
      const position = refined.get(node.id());
      if (position) node.position(position);
    });
  });
}

export function MapView({
  data,
  onSelect,
  onOpenEgo,
  onSelectEdge,
  showCandidates = false,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const weights = useUiStore((s) => s.weights);
  const topicEnabled = useUiStore((s) => s.topicEnabled);
  const activeAspect = useUiStore((s) => s.activeAspect);
  const showAssertionEdges = useUiStore((s) => s.showAssertionEdges);
  const minSimScore = useUiStore((s) => s.minSimScore);
  const setLod = useUiStore((s) => s.setLod);
  const assertionEdges = visibleMapEdges(
    data.edges,
    showCandidates || showAssertionEdges,
  );

  /** Show only similarity edges whose score ≥ threshold (no relayout). */
  const applySimThreshold = (cy: Core, threshold: number, draw: boolean) => {
    cy.batch(() => {
      cy.edges('.similarity').forEach((e) => {
        const score = Number(e.data('score') ?? 0);
        const show = draw && score >= threshold;
        e.style('display', show ? 'element' : 'none');
      });
    });
  };

  useEffect(() => {
    if (!containerRef.current) return;

    const displayMap = buildDisplayScoreMap(
      data,
      activeAspect,
      weights,
      topicEnabled,
    );
    const layoutMap = buildLayoutScoreMap(
      data,
      activeAspect,
      weights,
      topicEnabled,
    );
    // Park only if a node has no display-level neighbors at all.
    const unconnected = unconnectedNodes(data.nodes, displayMap);
    const drawSimilarity = !!activeAspect;

    const elements: cytoscape.ElementDefinition[] = [];
    let parkIdx = 0;
    const PARK_COLS = 4;
    const PARK_X0 = 980;
    const PARK_STEP = 200;
    data.nodes.forEach((n) => {
      const pos = seedPosition(n.work_id);
      // Slightly wider seed scatter so fcose starts less bunched.
      const jittered = { x: pos.x * 1.6, y: pos.y * 1.6 };
      const locked = unconnected.has(n.work_id);
      let position = jittered;
      if (locked) {
        const col = parkIdx % PARK_COLS;
        const row = Math.floor(parkIdx / PARK_COLS);
        position = { x: PARK_X0 + col * PARK_STEP, y: -420 + row * 90 };
        parkIdx += 1;
      }
      elements.push({
        data: {
          id: n.work_id,
          label: n.title.length > 40 ? n.title.slice(0, 38) + '…' : n.title,
          readers: n.readers,
          has_dispute: n.has_dispute || undefined,
          year: n.year,
          border_w: readerBorderWidth(n.readers),
        },
        position,
        locked,
      });
    });

    // Only sparse layout springs go into the graph *before* fcose —
    // denser visual edges are added after layout so they don't collapse spacing.
    for (const spring of buildLayoutSprings(layoutMap)) {
      elements.push({
        data: {
          id: `spring-${activeAspect ?? 'mix'}-${spring.key}`,
          source: spring.sourceId,
          target: spring.targetId,
          weight: spring.score,
          score: spring.score,
          primary: spring.primary,
          bestRank: spring.bestRank,
          idealEdgeLength: spring.idealLength,
          edgeElasticity: spring.elasticity,
          type: 'layout_spring',
        },
        classes: 'layout-spring',
      });
    }

    // Precompute visual edges to add after layout.
    const visualEdges: cytoscape.ElementDefinition[] = [];
    const edgeSet = new Set<string>();
    for (const [src, m] of displayMap) {
      for (const [tgt, score] of m) {
        const key = [src, tgt].sort().join('|');
        if (edgeSet.has(key)) continue;
        edgeSet.add(key);
        visualEdges.push({
          data: {
            id: `sim-${activeAspect ?? 'mix'}-${key}`,
            source: src,
            target: tgt,
            weight: score,
            score,
            type: 'similarity',
            label: score.toFixed(2),
          },
          classes: 'similarity',
        });
      }
    }

    if (showAssertionEdges) {
      const bundles = bundleEdges(
        assertionEdges.map((e) => ({
          relation_id: e.relation_id,
          source_id: e.source_work_id,
          target_id: e.target_work_id,
          relation_type: e.relation_type,
          review_status: e.review_status,
          source_layer: e.source_layer,
          confidence: e.confidence,
          explanation: e.explanation,
          review_count: e.review_count,
        })),
      );
      for (const b of bundles) {
        visualEdges.push({
          data: {
            id: `bundle-${b.key}`,
            source: b.source_id,
            target: b.target_id,
            label: b.label,
            status: b.status,
            type: b.semantic_group,
            review_count: b.review_count,
            relation_ids: b.relation_ids,
            count: b.count,
            kind: 'assertion',
          },
          classes: 'assertion',
        });
      }
    }

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        ...cyStylesheet,
        {
          selector: 'node[border_w]',
          style: {
            'border-width': 'data(border_w)',
            'border-color': COLORS.readerBorder,
          },
        },
        {
          selector: 'node[readers = 0]',
          style: {
            'border-color': COLORS.nodeBorder,
            'border-width': 1.5,
          },
        },
        {
          // Keep springs transparent rather than display:none: fCoSE excludes
          // display:none edges from its force calculation.
          selector: 'edge.layout-spring',
          style: {
            'curve-style': 'haystack',
            width: 0.1,
            opacity: 0,
            'line-opacity': 0,
            'target-arrow-shape': 'none',
            events: 'no',
          },
        },
        {
          selector: 'edge.similarity',
          style: {
            display: 'none', // applySimThreshold turns visible ones on
            // Bezier + control points reduce edge-on-node stacking vs haystack.
            'curve-style': 'unbundled-bezier',
            'control-point-distances': 28,
            'control-point-weights': 0.5,
            // Width tracks similarity — stronger pairs read as firmer threads.
            width: 'mapData(score, 0.25, 0.9, 0.7, 2.2)',
            'line-color': COLORS.simEdge,
            'line-opacity': 0.28,
            'target-arrow-shape': 'none',
            'line-style': 'solid',
            label: '',
            'font-family': GRAPH_FONT,
            'font-size': 7,
            color: COLORS.labelMuted,
          },
        },
        {
          selector: 'edge.similarity:selected, edge.similarity.hovered',
          style: {
            label: 'data(label)',
            'line-color': COLORS.simEdgeHover,
            'line-opacity': 0.95,
            width: 2,
            'z-index': 99,
            'text-background-color': COLORS.labelOutline,
            'text-background-opacity': 0.92,
            'text-background-padding': 2,
            'text-background-shape': 'roundrectangle',
          },
        },
        {
          selector: 'edge.assertion',
          style: {
            display: 'none',
            'curve-style': 'bezier',
            'target-arrow-shape': 'triangle',
            'arrow-scale': 0.7,
          },
        },
        {
          selector: 'edge[count > 1]',
          style: {
            width: 2,
            'line-opacity': 0.6,
          },
        },
      ] as cytoscape.StylesheetStyle[],
      layout: {
        name: 'fcose',
        animate: false,
        randomize: true,
        quality: 'proof',
        samplingType: true,
        sampleSize: 25,
        idealEdgeLength: (edge: cytoscape.EdgeSingular) =>
          edge.data('idealEdgeLength') ?? FCOSE_SPREAD.idealDefault,
        edgeElasticity: (edge: cytoscape.EdgeSingular) =>
          edge.data('edgeElasticity') ?? 0.03,
        nodeRepulsion: FCOSE_SPREAD.nodeRepulsion,
        nodeSeparation: FCOSE_SPREAD.nodeSeparation,
        gravity: FCOSE_SPREAD.gravity,
        gravityRange: FCOSE_SPREAD.gravityRange,
        numIter: FCOSE_SPREAD.numIter,
        packComponents: FCOSE_SPREAD.packComponents,
        tile: true,
      } as cytoscape.LayoutOptions,
      minZoom: 0.15,
      maxZoom: 3,
      wheelSensitivity: 0.3,
      textureOnViewport: true,
      hideEdgesOnViewport: true,
    });

    // fCoSE creates the global clusters; this ordinal pass fixes local cases
    // where a weaker node accidentally ends up closer than the strongest one.
    refineGlobalLayout(cy, layoutMap);
    cy.fit(cy.nodes(), 40);

    // Add denser visual edges *after* fcose so they don't act as springs.
    if (visualEdges.length > 0) {
      cy.add(visualEdges);
      // Slightly different bezier bows so parallel edges don't sit on one ray.
      cy.edges('.similarity').forEach((e) => {
        let h = 0;
        const id = e.id();
        for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0;
        const dist = 18 + Math.abs(h % 55);
        const sign = h & 1 ? 1 : -1;
        e.style({
          'control-point-distances': sign * dist,
          'control-point-weights': 0.35 + (Math.abs(h >> 3) % 30) / 100,
        });
      });
    }

    const applyLod = (zoom: number) => {
      const lod = lodFromZoom(zoom);
      setLod(lod);
      cy.batch(() => {
        if (lod === 'far') {
          cy.nodes().style('label', '');
          cy.edges('.assertion').style('display', 'none');
          if (drawSimilarity) {
            cy.edges('.similarity').forEach((e) => {
              if (e.style('display') === 'element') e.style('line-opacity', 0.15);
            });
          }
        } else if (lod === 'mid') {
          cy.nodes().forEach((n) => {
            n.style('label', n.data('label'));
          });
          cy.edges('.assertion').style('display', 'none');
          if (drawSimilarity) {
            cy.edges('.similarity').forEach((e) => {
              if (e.style('display') === 'element') e.style('line-opacity', 0.35);
            });
          }
        } else {
          cy.nodes().forEach((n) => {
            n.style('label', n.data('label'));
          });
          if (showAssertionEdges) {
            cy.edges('.assertion').style('display', 'element');
          }
          if (drawSimilarity) {
            cy.edges('.similarity').forEach((e) => {
              if (e.style('display') === 'element') e.style('line-opacity', 0.45);
            });
          }
        }
      });
    };

    cy.on('tap', 'node', (evt) => {
      onSelect(evt.target.id());
    });
    cy.on('dbltap', 'node', (evt) => {
      onOpenEgo(evt.target.id());
    });
    cy.on('tap', 'edge.assertion', (evt) => {
      const ids = evt.target.data('relation_ids') as string[] | undefined;
      const first = ids && ids.length > 0 ? ids[0] : evt.target.id();
      onSelectEdge?.(first);
    });
    cy.on('mouseover', 'edge.similarity', (evt) => {
      evt.target.addClass('hovered');
    });
    cy.on('mouseout', 'edge.similarity', (evt) => {
      evt.target.removeClass('hovered');
    });
    cy.on('mouseover', 'edge.assertion', (evt) => {
      evt.target.addClass('hovered');
    });
    cy.on('mouseout', 'edge.assertion', (evt) => {
      evt.target.removeClass('hovered');
    });
    cy.on('mouseover', 'node', (evt) => {
      const node = evt.target;
      const connected = node.connectedEdges();
      connected.addClass('hovered');
      cy.batch(() => {
        cy.edges().not(connected).style('line-opacity', 0.06);
      });
    });
    cy.on('mouseout', 'node', () => {
      cy.batch(() => {
        cy.edges().removeClass('hovered');
        cy.edges('.similarity').style('line-opacity', '');
        cy.edges('.assertion').style('line-opacity', '');
      });
      applyLod(cy.zoom());
    });
    cy.on('zoom', () => applyLod(cy.zoom()));
    applySimThreshold(cy, minSimScore, drawSimilarity);
    applyLod(cy.zoom());

    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
    // Re-init when data / aspect / assertion overlay changes (not minSimScore).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, activeAspect, showAssertionEdges, showCandidates]);

  // Live filter: only toggle edge display when the similarity threshold moves.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    applySimThreshold(cy, minSimScore, !!activeAspect);
  }, [minSimScore, activeAspect]);

  // Re-layout on weight change without full remount (legacy mode only).
  const previousLegacyConfigRef = useRef<string | null>(null);
  useEffect(() => {
    if (activeAspect) {
      previousLegacyConfigRef.current = null;
      return;
    }
    const configKey = JSON.stringify({ weights, topicEnabled });
    if (previousLegacyConfigRef.current === null) {
      previousLegacyConfigRef.current = configKey;
      return;
    }
    if (previousLegacyConfigRef.current === configKey) return;
    previousLegacyConfigRef.current = configKey;

    const cy = cyRef.current;
    if (!cy) return;
    const layoutMap = buildLayoutScoreMap(data, null, weights, topicEnabled);
    const springs = buildLayoutSprings(layoutMap);
    cy.batch(() => {
      cy.remove(cy.edges('.layout-spring'));
      cy.add(
        springs.map((spring) => ({
          data: {
            id: `spring-mix-${spring.key}`,
            source: spring.sourceId,
            target: spring.targetId,
            weight: spring.score,
            score: spring.score,
            primary: spring.primary,
            bestRank: spring.bestRank,
            idealEdgeLength: spring.idealLength,
            edgeElasticity: spring.elasticity,
            type: 'layout_spring',
          },
          classes: 'layout-spring',
        })),
      );
    });
    cy.one('layoutstop', () => {
      refineGlobalLayout(cy, layoutMap);
      cy.fit(cy.nodes(), 40);
    });
    cy.layout({
      name: 'fcose',
      animate: true,
      animationDuration: 500,
      randomize: false,
      quality: 'proof',
      idealEdgeLength: (edge: cytoscape.EdgeSingular) =>
        edge.data('idealEdgeLength') ?? FCOSE_SPREAD.idealDefault,
      edgeElasticity: (edge: cytoscape.EdgeSingular) =>
        edge.data('edgeElasticity') ?? 0.03,
      nodeRepulsion: FCOSE_SPREAD.nodeRepulsion,
      nodeSeparation: FCOSE_SPREAD.nodeSeparation,
      gravity: FCOSE_SPREAD.gravity,
      gravityRange: FCOSE_SPREAD.gravityRange,
      numIter: FCOSE_SPREAD.numIter,
      packComponents: FCOSE_SPREAD.packComponents,
      tile: true,
    } as cytoscape.LayoutOptions).run();
  }, [weights, topicEnabled, data, activeAspect]);

  return <div ref={containerRef} className="cy-container cy-canvas" />;
}
