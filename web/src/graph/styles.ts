/** Shared Cytoscape style tokens (card / queue / legend reuse these).
 *  Values follow the MD3 light scheme in index.css (blue seed #0B57D0). */

export const GRAPH_FONT = 'Roboto, "Noto Sans SC", system-ui, sans-serif';

export const COLORS = {
  node: '#2e3036', // touched by the team (inverse-surface)
  nodeUnread: '#b0b4c0', // deepened outline-variant — nobody has read it
  nodeSelected: '#0b57d0', // primary
  nodeSelectedRing: '#a8c7fa', // blue80 halo around selection
  nodeBorder: '#f9f9ff', // surface — crisp separation ring
  readerBorder: '#0b57d0', // reader-count border (primary)
  label: '#191c20', // on-surface
  labelMuted: '#44474e', // on-surface-variant
  labelOutline: '#f9f9ff', // surface halo behind labels
  // Assertion edges: review strength = one blue, light→dark (ordinal ramp);
  // grey dashes = undecided candidate; error red = disputed.
  edgeCandidate: '#74777f', // outline
  edgeConfirmed1: '#4c8df6', // blue60
  edgeConfirmed2: '#0b57d0', // blue40
  edgeDisputed: '#b3261e', // error
  disputeDot: '#b3261e',
  // Similarity edges are recessive context, never data marks.
  simEdge: '#a8c7fa', // blue80
  simEdgeHover: '#1b6ef3', // blue50
  // Ego overflow group nodes
  groupFill: '#e7e8ee', // surface-container-high
  groupBorder: '#74777f',
  groupText: '#44474e',
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
      'font-family': GRAPH_FONT,
      'font-size': 8,
      'text-valign': 'bottom',
      'text-halign': 'center',
      'text-margin-y': 5,
      'text-max-width': 110,
      'text-wrap': 'ellipsis',
      'text-outline-width': 2.5,
      'text-outline-color': COLORS.labelOutline,
      'text-outline-opacity': 0.95,
      'min-zoomed-font-size': 8,
      width: 13,
      height: 13,
      'border-width': 1.5,
      'border-color': COLORS.nodeBorder,
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
      'border-color': COLORS.nodeSelectedRing,
      'border-width': 4,
      width: 18,
      height: 18,
      'font-size': 9,
      'z-index': 100,
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
      'line-opacity': 0.45,
      'target-arrow-color': COLORS.edgeCandidate,
      'target-arrow-shape': 'triangle',
      'arrow-scale': 0.8,
      'curve-style': 'straight',
      'line-style': 'dashed',
      'line-dash-pattern': [5, 4],
      'font-family': GRAPH_FONT,
      'font-size': 7,
      color: COLORS.labelMuted,
      'text-rotation': 'autorotate',
      'text-background-color': COLORS.labelOutline,
      'text-background-opacity': 0.92,
      'text-background-padding': 2,
      'text-background-shape': 'roundrectangle',
      'min-zoomed-font-size': 9,
    },
  },
  {
    // Only show the relation-type label on hover / selection to avoid a text hairball.
    selector: 'edge:selected, edge.hovered',
    style: {
      label: 'data(label)',
      'line-opacity': 1,
      width: 2.5,
      'z-index': 99,
    },
  },
  {
    selector: 'edge[status = "confirmed"]',
    style: {
      'line-style': 'solid',
      'line-opacity': 0.8,
      'line-color': COLORS.edgeConfirmed1,
      'target-arrow-color': COLORS.edgeConfirmed1,
      width: 2,
    },
  },
  {
    selector: 'edge[status = "confirmed"][review_count >= 2]',
    style: {
      'line-color': COLORS.edgeConfirmed2,
      'target-arrow-color': COLORS.edgeConfirmed2,
      width: 2.5,
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
