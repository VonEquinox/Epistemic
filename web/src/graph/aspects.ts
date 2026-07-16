/** Fixed multi-aspect DNA layers (must match server ASPECTS). */

export interface AspectDef {
  key: string;
  label: string;
  /** neighbors map key = NeighborDimension snake_case */
  dimension: string;
}

export const ASPECTS: AspectDef[] = [
  { key: 'problem', label: '问题设定', dimension: 'aspect_problem' },
  { key: 'contributions', label: '贡献', dimension: 'aspect_contributions' },
  { key: 'methods', label: '方法', dimension: 'aspect_methods' },
  { key: 'theory', label: '理论/形式化', dimension: 'aspect_theory' },
  { key: 'datasets', label: '数据与基准', dimension: 'aspect_datasets' },
  { key: 'findings', label: '主张与结果', dimension: 'aspect_findings' },
  { key: 'limitations', label: '局限', dimension: 'aspect_limitations' },
  { key: 'positioning', label: '相关工作定位', dimension: 'aspect_positioning' },
];

export function aspectByKey(key: string): AspectDef | undefined {
  return ASPECTS.find((a) => a.key === key);
}

/** Neighbors kept for *visible* similarity edges (data stores top-32). */
export const ASPECT_EDGE_TOP_K = 12;
/**
 * Floor when building display edges. Live minSimScore slider filters further.
 */
export const ASPECT_EDGE_BUILD_MIN = 0.28;
/** Default for the min-similarity slider. */
export const ASPECT_EDGE_MIN_SCORE = 0.5;
/**
 * Sparser top-K used only as fcose springs so the map spreads out.
 * Display edges can be denser without collapsing the layout.
 */
export const ASPECT_LAYOUT_TOP_K = 4;
export const ASPECT_LAYOUT_MIN_SCORE = 0.4;
