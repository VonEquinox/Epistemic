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
      'font-size': 10,
      'text-valign': 'bottom',
      'text-halign': 'center',
      'text-margin-y': 6,
      width: 12,
      height: 12,
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
      width: 1.5,
      'line-color': COLORS.edgeCandidate,
      'target-arrow-color': COLORS.edgeCandidate,
      'target-arrow-shape': 'triangle',
      'curve-style': 'bezier',
      'line-style': 'dashed',
      label: 'data(label)',
      'font-size': 8,
      color: '#6b6b5e',
      'text-rotation': 'autorotate',
    },
  },
  {
    selector: 'edge[status = "confirmed"]',
    style: {
      'line-style': 'solid',
      'line-color': COLORS.edgeConfirmed1,
      'target-arrow-color': COLORS.edgeConfirmed1,
    },
  },
  {
    selector: 'edge[status = "disputed"]',
    style: {
      'line-style': 'solid',
      'line-color': COLORS.edgeDisputed,
      'target-arrow-color': COLORS.edgeDisputed,
      width: 3,
    },
  },
  {
    selector: 'edge[type = "cites"]',
    style: {
      display: 'none',
    },
  },
];
