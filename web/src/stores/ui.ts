import { create } from 'zustand';

export type LodLevel = 'far' | 'mid' | 'near';

interface UiState {
  weights: { citation_coupling: number; method_lineage: number; topic: number };
  topicEnabled: boolean;
  /** Active multi-aspect layer key (e.g. methods). null = legacy combined layout. */
  activeAspect: string | null;
  /** Overlay assertion edges (LLM pairs / reviews) on the aspect similarity map. */
  showAssertionEdges: boolean;
  selectedWorkId: string | null;
  drawerOpen: boolean;
  lod: LodLevel;
  setWeights: (w: Partial<UiState['weights']>) => void;
  setTopicEnabled: (v: boolean) => void;
  setActiveAspect: (key: string | null) => void;
  setShowAssertionEdges: (v: boolean) => void;
  selectWork: (id: string | null) => void;
  setLod: (l: LodLevel) => void;
}

export const useUiStore = create<UiState>((set) => ({
  weights: { citation_coupling: 0.6, method_lineage: 0.4, topic: 0 },
  topicEnabled: false,
  activeAspect: 'methods',
  showAssertionEdges: false,
  selectedWorkId: null,
  drawerOpen: false,
  lod: 'mid',
  setWeights: (w) => set((s) => ({ weights: { ...s.weights, ...w } })),
  setTopicEnabled: (v) => set({ topicEnabled: v }),
  setActiveAspect: (key) => set({ activeAspect: key }),
  setShowAssertionEdges: (v) => set({ showAssertionEdges: v }),
  selectWork: (id) => set({ selectedWorkId: id, drawerOpen: !!id }),
  setLod: (l) => set({ lod: l }),
}));
