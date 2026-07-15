import { useEffect, useRef } from 'react';
import cytoscape, { type Core } from 'cytoscape';
// @ts-expect-error no types
import fcose from 'cytoscape-fcose';
import type { EgoResponse } from '../api/types';
import { cyStylesheet } from './styles';
import { bundleEdges, seedPosition } from './layout';

cytoscape.use(fcose);

interface Props {
  data: EgoResponse;
  /** Called with relation ids in a bundle (may be >1). */
  onSelectBundle?: (relationIds: string[]) => void;
  onSelectNode?: (id: string, kind: string, groupKey?: string) => void;
}

export function EgoView({ data, onSelectBundle, onSelectNode }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const elements: cytoscape.ElementDefinition[] = data.nodes.map((n) => ({
      data: {
        id: n.id,
        label:
          n.kind === 'group'
            ? n.label
            : n.label.length > 36
              ? n.label.slice(0, 34) + '…'
              : n.label,
        kind: n.kind,
        group_key: n.group_key,
        group_count: n.group_count,
      },
      position: seedPosition(n.id),
      classes: n.kind === 'group' ? 'group' : undefined,
    }));

    const groupNodeIds = new Set(
      data.nodes.filter((n) => n.kind === 'group').map((n) => n.id),
    );
    const normalEdges = data.edges.filter(
      (e) => !groupNodeIds.has(e.source_id) && !groupNodeIds.has(e.target_id),
    );
    const groupEdges = data.edges.filter(
      (e) => groupNodeIds.has(e.source_id) || groupNodeIds.has(e.target_id),
    );

    const bundles = bundleEdges(normalEdges);
    for (const b of bundles) {
      elements.push({
        data: {
          id: `bundle-${b.key}`,
          source: b.source_id,
          target: b.target_id,
          label: b.count > 1 ? b.label : b.label,
          status: b.status,
          type: b.semantic_group,
          review_count: b.review_count,
          relation_ids: b.relation_ids,
          count: b.count,
        },
      });
    }
    for (const e of groupEdges) {
      elements.push({
        data: {
          id: e.relation_id,
          source: e.source_id,
          target: e.target_id,
          label: e.explanation || e.relation_type,
          status: e.review_status,
          type: e.relation_type,
          review_count: 0,
          relation_ids: [],
          count: 0,
        },
        classes: 'group-edge',
      });
    }

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        ...cyStylesheet,
        {
          selector: 'node.group',
          style: {
            'background-color': '#e7e5e4',
            'border-color': '#a8a29e',
            'border-width': 2,
            'border-style': 'dashed',
            shape: 'round-rectangle',
            width: 28,
            height: 20,
            'font-size': 9,
            color: '#57534e',
          },
        },
        {
          selector: 'edge[count > 1]',
          style: {
            width: 2.5,
            label: 'data(label)',
            'font-size': 9,
          },
        },
      ] as cytoscape.StylesheetStyle[],
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

    cy.on('tap', 'edge', (evt) => {
      const ids = evt.target.data('relation_ids') as string[] | undefined;
      if (ids && ids.length > 0) onSelectBundle?.(ids);
    });
    cy.on('tap', 'node', (evt) =>
      onSelectNode?.(
        evt.target.id(),
        evt.target.data('kind'),
        evt.target.data('group_key'),
      ),
    );

    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [data, onSelectBundle, onSelectNode]);

  return (
    <div
      ref={containerRef}
      className="cy-container bg-white rounded-lg border border-ink-200"
    />
  );
}
