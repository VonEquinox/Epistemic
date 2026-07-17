import { create } from 'zustand';
import { DEFAULT_FORCE_TUNING, type ForceSimulationConfig } from '../graph/layout';

export type LodLevel = 'far' | 'mid' | 'near';

interface UiState {
  weights: { citation_coupling: number; method_lineage: number; topic: number };
  topicEnabled: boolean;
  /** Active multi-aspect layer key (e.g. methods). null = legacy combined layout. */
  activeAspect: string | null;
  /** Overlay assertion edges (LLM pairs / reviews) on the aspect similarity map. */
  showAssertionEdges: boolean;
  /**
   * Minimum cosine similarity for visible aspect edges (0–1).
   * Higher = only stronger / “closer” pairs. Applied live without relayout.
   */
  minSimScore: number;
  forceTuning: ForceSimulationConfig;
  /** Active group / graph workspace for navigation. */
  activeGroupId: string | null;
  activeGraphId: string | null;
  selectedWorkId: string | null;
  drawerOpen: boolean;
  lod: LodLevel;
  setWeights: (w: Partial<UiState['weights']>) => void;
  setTopicEnabled: (v: boolean) => void;
  setActiveAspect: (key: string | null) => void;
  setShowAssertionEdges: (v: boolean) => void;
  setMinSimScore: (v: number) => void;
  setForceTuning: (value: Partial<ForceSimulationConfig>) => void;
  resetForceTuning: () => void;
  setActiveGroupId: (id: string | null) => void;
  setActiveGraphId: (id: string | null) => void;
  selectWork: (id: string | null) => void;
  setLod: (l: LodLevel) => void;
}

export const useUiStore = create<UiState>((set) => ({
  weights: { citation_coupling: 0.6, method_lineage: 0.4, topic: 0 },
  topicEnabled: false,
  activeAspect: 'methods',
  showAssertionEdges: false,
  minSimScore: 0.5,
  forceTuning: { ...DEFAULT_FORCE_TUNING },
  activeGroupId: null,
  activeGraphId: null,
  selectedWorkId: null,
  drawerOpen: false,
  lod: 'mid',
  setWeights: (w) => set((s) => ({ weights: { ...s.weights, ...w } })),
  setTopicEnabled: (v) => set({ topicEnabled: v }),
  setActiveAspect: (key) => set({ activeAspect: key }),
  setShowAssertionEdges: (v) => set({ showAssertionEdges: v }),
  setMinSimScore: (v) =>
    set({ minSimScore: Math.max(0, Math.min(1, Number.isFinite(v) ? v : 0.5)) }),
  setForceTuning: (value) =>
    set((state) => ({ forceTuning: { ...state.forceTuning, ...value } })),
  resetForceTuning: () => set({ forceTuning: { ...DEFAULT_FORCE_TUNING } }),
  setActiveGroupId: (id) => set({ activeGroupId: id }),
  setActiveGraphId: (id) => set({ activeGraphId: id }),
  selectWork: (id) => set({ selectedWorkId: id, drawerOpen: !!id }),
  setLod: (l) => set({ lod: l }),
}));
