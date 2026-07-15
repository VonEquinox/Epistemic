import type { RelationType, ReviewStatus } from '../api/types';

const typeLabel: Record<RelationType, string> = {
  cites: '引用',
  version_of: '版本',
  uses_method_from: '使用方法',
  improves_on: '改进',
  alternative_to: '平行方法',
  uses_dataset_from: '使用数据集',
  compares_against: '对比',
  reproduces: '复现',
  fails_to_reproduce: '复现失败',
  supports_claim: '支持主张',
  contradicts_claim: '反驳主张',
  prerequisite_for: '前置阅读',
};

const statusCls: Record<ReviewStatus, string> = {
  unreviewed: 'border-dashed border-ink-300 text-ink-500',
  confirmed: 'border-solid border-blue-300 text-blue-700 bg-blue-50',
  disputed: 'border-solid border-rose-400 text-rose-700 bg-rose-50',
  rejected: 'border-solid border-ink-200 text-ink-400 line-through',
};

export function RelationBadge({
  type,
  status,
}: {
  type: RelationType;
  status: ReviewStatus;
}) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs border ${statusCls[status]}`}
    >
      {typeLabel[type] ?? type}
    </span>
  );
}
