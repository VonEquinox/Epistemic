import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
// @ts-expect-error no types
import fcose from 'cytoscape-fcose';
import type { MapEdge, MapResponse } from '../api/types';
import { useUiStore } from '../stores/ui';
import { cyStylesheet } from './styles';
import {
  aspectNeighborMap,
  combineNeighbors,
  seedPosition,
  springLength,
  unconnectedNodes,
  lodFromZoom,
  readerBorderWidth,
  bundleEdges,
} from './layout';
import {
  ASPECT_EDGE_MIN_SCORE,
  ASPECT_EDGE_TOP_K,
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

function buildScoreMap(
  data: MapResponse,
  activeAspect: string | null,
  weights: { citation_coupling: number; method_lineage: number; topic: number },
  topicEnabled: boolean,
): Map<string, Map<string, number>> {
  if (activeAspect) {
    const def = aspectByKey(activeAspect);
    const dim = def?.dimension ?? `aspect_${activeAspect}`;
    return aspectNeighborMap(
      data.neighbors,
      dim,
      ASPECT_EDGE_TOP_K,
      ASPECT_EDGE_MIN_SCORE,
    );
  }
  return combineNeighbors(data.neighbors, weights, topicEnabled);
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
  const setLod = useUiStore((s) => s.setLod);
  const assertionEdges = visibleMapEdges(
    data.edges,
    showCandidates || showAssertionEdges,
  );

  useEffect(() => {
    if (!containerRef.current) return;

    const scoreMap = buildScoreMap(data, activeAspect, weights, topicEnabled);
    const unconnected = unconnectedNodes(data.nodes, scoreMap);
    const drawSimilarity = !!activeAspect;

    const elements: cytoscape.ElementDefinition[] = [];
    let parkIdx = 0;
    const PARK_COLS = 4;
    const PARK_X0 = 640;
    const PARK_STEP = 150;
    data.nodes.forEach((n) => {
      const pos = seedPosition(n.work_id);
      const locked = unconnected.has(n.work_id);
      let position = pos;
      if (locked) {
        const col = parkIdx % PARK_COLS;
        const row = Math.floor(parkIdx / PARK_COLS);
        position = { x: PARK_X0 + col * PARK_STEP, y: -320 + row * 60 };
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

    // Similarity / spring edges from active aspect (or combined weights).
    const edgeSet = new Set<string>();
    for (const [src, m] of scoreMap) {
      for (const [tgt, score] of m) {
        const key = [src, tgt].sort().join('|');
        if (edgeSet.has(key)) continue;
        edgeSet.add(key);
        elements.push({
          data: {
            id: `sim-${activeAspect ?? 'mix'}-${key}`,
            source: src,
            target: tgt,
            weight: score,
            score,
            idealEdgeLength: springLength(score),
            type: 'similarity',
            label: score.toFixed(2),
          },
          classes: 'similarity',
        });
      }
    }

    // Assertion edges (optional overlay)
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
        elements.push({
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
            'border-color': '#2563eb',
          },
        },
        {
          selector: 'node[readers = 0]',
          style: {
            'border-color': '#fff',
            'border-width': 2,
          },
        },
        {
          selector: 'edge.similarity',
          style: {
            display: drawSimilarity ? 'element' : 'none',
            'curve-style': 'haystack',
            'haystack-radius': 0.4,
            width: 1.2,
            'line-color': '#94a3b8',
            'line-opacity': 0.35,
            'target-arrow-shape': 'none',
            'line-style': 'solid',
            label: '',
            'font-size': 7,
            color: '#64748b',
          },
        },
        {
          selector: 'edge.similarity:selected, edge.similarity.hovered',
          style: {
            label: 'data(label)',
            'line-opacity': 0.9,
            width: 2,
            'z-index': 99,
            'text-background-color': '#ffffff',
            'text-background-opacity': 0.9,
            'text-background-padding': 2,
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
        randomize: false,
        quality: 'proof',
        idealEdgeLength: (edge: cytoscape.EdgeSingular) =>
          edge.data('idealEdgeLength') ?? 200,
        nodeRepulsion: () => 24000,
        nodeSeparation: 120,
        gravity: 0.15,
        gravityRange: 4.0,
        numIter: 3000,
        packing: 'true',
      } as cytoscape.LayoutOptions,
      minZoom: 0.2,
      maxZoom: 3,
      wheelSensitivity: 0.3,
      textureOnViewport: true,
      hideEdgesOnViewport: true,
    });

    const applyLod = (zoom: number) => {
      const lod = lodFromZoom(zoom);
      setLod(lod);
      cy.batch(() => {
        if (lod === 'far') {
          cy.nodes().style('label', '');
          cy.edges('.assertion').style('display', 'none');
          if (drawSimilarity) {
            cy.edges('.similarity').style('line-opacity', 0.15);
          }
        } else if (lod === 'mid') {
          cy.nodes().forEach((n) => {
            n.style('label', n.data('label'));
          });
          cy.edges('.assertion').style('display', 'none');
          if (drawSimilarity) {
            cy.edges('.similarity').style('line-opacity', 0.35);
          }
        } else {
          cy.nodes().forEach((n) => {
            n.style('label', n.data('label'));
          });
          if (showAssertionEdges) {
            cy.edges('.assertion').style('display', 'element');
          }
          if (drawSimilarity) {
            cy.edges('.similarity').style('line-opacity', 0.45);
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
    applyLod(cy.zoom());

    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
    // Re-init when data / aspect / assertion overlay changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, activeAspect, showAssertionEdges, showCandidates]);

  // Re-layout on weight change without full remount (legacy mode only)
  useEffect(() => {
    if (activeAspect) return;
    const cy = cyRef.current;
    if (!cy) return;
    const combined = combineNeighbors(data.neighbors, weights, topicEnabled);
    cy.batch(() => {
      cy.edges('.similarity').forEach((e) => {
        const src = e.source().id();
        const tgt = e.target().id();
        const score =
          combined.get(src)?.get(tgt) ?? combined.get(tgt)?.get(src) ?? 0;
        e.data('idealEdgeLength', springLength(score));
      });
    });
    cy.layout({
      name: 'fcose',
      animate: true,
      animationDuration: 400,
      randomize: false,
      idealEdgeLength: (edge: cytoscape.EdgeSingular) =>
        edge.data('idealEdgeLength') ?? 200,
      nodeRepulsion: () => 24000,
      nodeSeparation: 120,
      gravity: 0.15,
    } as cytoscape.LayoutOptions).run();
  }, [weights, topicEnabled, data.neighbors, activeAspect]);

  return (
    <div
      ref={containerRef}
      className="cy-container bg-white rounded-lg border border-ink-200"
    />
  );
}
