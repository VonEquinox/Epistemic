import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
// @ts-expect-error no types
import fcose from 'cytoscape-fcose';
import type { MapResponse } from '../api/types';
import { useUiStore } from '../stores/ui';
import { cyStylesheet } from './styles';
import {
  combineNeighbors,
  seedPosition,
  springLength,
  unconnectedNodes,
  lodFromZoom,
} from './layout';

cytoscape.use(fcose);

interface Props {
  data: MapResponse;
  onSelect: (workId: string) => void;
  onOpenEgo: (workId: string) => void;
}

export function MapView({ data, onSelect, onOpenEgo }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);
  const weights = useUiStore((s) => s.weights);
  const topicEnabled = useUiStore((s) => s.topicEnabled);
  const setLod = useUiStore((s) => s.setLod);

  useEffect(() => {
    if (!containerRef.current) return;

    const combined = combineNeighbors(data.neighbors, weights, topicEnabled);
    const unconnected = unconnectedNodes(data.nodes, combined);

    const elements: cytoscape.ElementDefinition[] = [];
    data.nodes.forEach((n, i) => {
      const pos = seedPosition(n.work_id);
      const locked = unconnected.has(n.work_id);
      elements.push({
        data: {
          id: n.work_id,
          label: n.title.length > 40 ? n.title.slice(0, 38) + '…' : n.title,
          readers: n.readers,
          has_dispute: n.has_dispute || undefined,
          year: n.year,
        },
        position: locked
          ? { x: 420, y: -200 + i * 18 }
          : pos,
        locked,
      });
    });

    // Ideal edges for fcose from combined scores (not visual assertion edges)
    const edgeSet = new Set<string>();
    for (const [src, m] of combined) {
      for (const [tgt, score] of m) {
        const key = [src, tgt].sort().join('|');
        if (edgeSet.has(key)) continue;
        edgeSet.add(key);
        elements.push({
          data: {
            id: `sim-${key}`,
            source: src,
            target: tgt,
            weight: score,
            idealEdgeLength: springLength(score),
            type: 'similarity',
          },
          classes: 'similarity',
        });
      }
    }

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        ...cyStylesheet,
        {
          selector: 'edge.similarity',
          style: { display: 'none' }, // layout-only edges
        },
      ] as cytoscape.StylesheetStyle[],
      layout: {
        name: 'fcose',
        animate: false,
        randomize: false,
        quality: 'default',
        idealEdgeLength: (edge: cytoscape.EdgeSingular) =>
          edge.data('idealEdgeLength') ?? 120,
        nodeRepulsion: () => 4500,
        packing: 'true',
      } as cytoscape.LayoutOptions,
      minZoom: 0.2,
      maxZoom: 3,
      wheelSensitivity: 0.3,
      textureOnViewport: true,
      hideEdgesOnViewport: true,
    });

    cy.on('tap', 'node', (evt) => {
      onSelect(evt.target.id());
    });
    cy.on('dbltap', 'node', (evt) => {
      onOpenEgo(evt.target.id());
    });
    cy.on('zoom', () => {
      const lod = lodFromZoom(cy.zoom());
      setLod(lod);
      cy.batch(() => {
        if (lod === 'far') {
          cy.nodes().style('label', '');
        } else {
          cy.nodes().forEach((n) => {
            n.style('label', n.data('label'));
          });
        }
      });
    });

    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
    // re-init when data identity changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data]);

  // Re-layout on weight change without full remount
  useEffect(() => {
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
        edge.data('idealEdgeLength') ?? 120,
      nodeRepulsion: () => 4500,
    } as cytoscape.LayoutOptions).run();
  }, [weights, topicEnabled, data.neighbors]);

  return <div ref={containerRef} className="cy-container bg-white rounded-lg border border-ink-200" />;
}
