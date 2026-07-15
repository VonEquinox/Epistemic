import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
// @ts-expect-error no types
import fcose from 'cytoscape-fcose';
import type { EgoResponse } from '../api/types';
import { cyStylesheet } from './styles';
import { seedPosition } from './layout';

cytoscape.use(fcose);

interface Props {
  data: EgoResponse;
  onSelectEdge?: (relationId: string) => void;
  onSelectNode?: (id: string, kind: string) => void;
}

export function EgoView({ data, onSelectEdge, onSelectNode }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const elements: cytoscape.ElementDefinition[] = data.nodes.map((n) => ({
      data: {
        id: n.id,
        label: n.label.length > 36 ? n.label.slice(0, 34) + '…' : n.label,
        kind: n.kind,
      },
      position: seedPosition(n.id),
    }));

    for (const e of data.edges) {
      elements.push({
        data: {
          id: e.relation_id,
          source: e.source_id,
          target: e.target_id,
          label: e.relation_type.replace(/_/g, ' '),
          status: e.review_status,
          type: e.relation_type,
          review_count: e.review_count,
        },
      });
    }

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: cyStylesheet as cytoscape.StylesheetStyle[],
      layout: {
        name: 'fcose',
        animate: false,
        randomize: false,
        nodeRepulsion: () => 6000,
        idealEdgeLength: () => 100,
      } as cytoscape.LayoutOptions,
      minZoom: 0.3,
      maxZoom: 3,
      wheelSensitivity: 0.3,
    });

    cy.on('tap', 'edge', (evt) => onSelectEdge?.(evt.target.id()));
    cy.on('tap', 'node', (evt) =>
      onSelectNode?.(evt.target.id(), evt.target.data('kind')),
    );

    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [data, onSelectEdge, onSelectNode]);

  return <div ref={containerRef} className="cy-container bg-white rounded-lg border border-ink-200" />;
}
