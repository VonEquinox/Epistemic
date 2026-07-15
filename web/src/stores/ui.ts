import { create } from 'zustand';

export type LodLevel = 'far' | 'mid' | 'near';

interface UiState {
  weights: { citation_coupling: number; method_lineage: number; topic: number };
  topicEnabled: boolean;
  selectedWorkId: string | null;
  drawerOpen: boolean;
  lod: LodLevel;
  setWeights: (w: Partial<UiState['weights']>) => void;
  setTopicEnabled: (v: boolean) => void;
  selectWork: (id: string | null) => void;
  setLod: (l: LodLevel) => void;
}

export const useUiStore = create<UiState>((set) => ({
  weights: { citation_coupling: 0.6, method_lineage: 0.4, topic: 0 },
  topicEnabled: false,
  selectedWorkId: null,
  drawerOpen: false,
  lod: 'mid',
  setWeights: (w) => set((s) => ({ weights: { ...s.weights, ...w } })),
  setTopicEnabled: (v) => set({ topicEnabled: v }),
  selectWork: (id) => set({ selectedWorkId: id, drawerOpen: !!id }),
  setLod: (l) => set({ lod: l }),
}));
