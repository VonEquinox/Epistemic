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

/** Display top-K similarity edges on the map (data stores top-32). */
export const ASPECT_EDGE_TOP_K = 8;
/** Hide weak similarity edges below this score. */
export const ASPECT_EDGE_MIN_SCORE = 0.45;
