/** Shared Cytoscape style tokens (card / queue / legend reuse these). */

export const COLORS = {
  node: '#3a3a33',
  nodeUnread: '#b3b3a8',
  nodeSelected: '#2563eb',
  label: '#32322c',
  edgeCandidate: '#9ca3af',
  edgeConfirmed1: '#93c5fd',
  edgeConfirmed2: '#1d4ed8',
  edgeDisputed: '#dc2626',
  disputeDot: '#dc2626',
};

export function edgeStyle(status: string, reviewCount: number): {
  lineStyle: 'solid' | 'dashed';
  color: string;
  width: number;
} {
  if (status === 'disputed') {
    return { lineStyle: 'solid', color: COLORS.edgeDisputed, width: 3 };
  }
  if (status === 'confirmed') {
    if (reviewCount >= 2) {
      return { lineStyle: 'solid', color: COLORS.edgeConfirmed2, width: 2.5 };
    }
    return { lineStyle: 'solid', color: COLORS.edgeConfirmed1, width: 2 };
  }
  // candidate / unreviewed
  return { lineStyle: 'dashed', color: COLORS.edgeCandidate, width: 1.5 };
}

export const cyStylesheet = [
  {
    selector: 'node',
    style: {
      'background-color': COLORS.node,
      label: 'data(label)',
      color: COLORS.label,
      'font-size': 7,
      'text-valign': 'bottom',
      'text-halign': 'center',
      'text-margin-y': 4,
      'text-max-width': 96,
      'text-wrap': 'ellipsis',
      'text-outline-width': 2,
      'text-outline-color': '#ffffff',
      'min-zoomed-font-size': 7,
      width: 11,
      height: 11,
      'border-width': 2,
      'border-color': '#fff',
    },
  },
  {
    selector: 'node[readers = 0]',
    style: {
      'background-color': COLORS.nodeUnread,
    },
  },
  {
    selector: 'node:selected',
    style: {
      'background-color': COLORS.nodeSelected,
      'border-color': COLORS.nodeSelected,
      width: 16,
      height: 16,
    },
  },
  {
    selector: 'node[has_dispute]',
    style: {
      'border-color': COLORS.disputeDot,
      'border-width': 3,
    },
  },
  {
    selector: 'edge',
    style: {
      width: 1,
      'line-color': COLORS.edgeCandidate,
      'line-opacity': 0.4,
      'target-arrow-color': COLORS.edgeCandidate,
      'target-arrow-shape': 'triangle',
      'arrow-scale': 0.7,
      'curve-style': 'bezier',
      'line-style': 'dashed',
      'font-size': 7,
      color: '#6b6b5e',
      'text-rotation': 'autorotate',
      'text-background-color': '#ffffff',
      'text-background-opacity': 0.85,
      'text-background-padding': 1,
      'min-zoomed-font-size': 9,
    },
  },
  {
    // Only show the relation-type label on hover / selection to avoid a text hairball.
    selector: 'edge:selected, edge.hovered',
    style: {
      label: 'data(label)',
      'line-opacity': 1,
      width: 2,
      'z-index': 99,
    },
  },
  {
    selector: 'edge[status = "confirmed"]',
    style: {
      'line-style': 'solid',
      'line-opacity': 0.75,
      'line-color': COLORS.edgeConfirmed1,
      'target-arrow-color': COLORS.edgeConfirmed1,
    },
  },
  {
    selector: 'edge[status = "disputed"]',
    style: {
      'line-style': 'solid',
      'line-opacity': 1,
      'line-color': COLORS.edgeDisputed,
      'target-arrow-color': COLORS.edgeDisputed,
      width: 2.5,
    },
  },
  {
    selector: 'edge[type = "cites"]',
    style: {
      display: 'none',
    },
  },
];
