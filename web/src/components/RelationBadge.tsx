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

const typeCls: Record<RelationType, string> = {
  uses_method_from: 'bg-primary-container text-on-primary-container',
  improves_on: 'bg-primary-container text-on-primary-container',
  alternative_to: 'bg-primary-container text-on-primary-container',
  uses_dataset_from: 'bg-secondary-container text-on-secondary-container',
  compares_against: 'bg-secondary-container text-on-secondary-container',
  reproduces: 'bg-secondary-container text-on-secondary-container',
  supports_claim: 'bg-tertiary-container text-on-tertiary-container',
  fails_to_reproduce: 'bg-error-container text-on-error-container',
  contradicts_claim: 'bg-error-container text-on-error-container',
  prerequisite_for: 'bg-surface-container-high text-on-surface-variant',
  cites: 'bg-surface-container-high text-on-surface-variant',
  version_of: 'bg-surface-container-high text-on-surface-variant',
};

const statusCls: Record<ReviewStatus, string> = {
  unreviewed: 'opacity-70',
  confirmed: '',
  disputed: 'ring-1 ring-error',
  rejected: 'opacity-50 line-through',
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
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${
        typeCls[type] ?? 'bg-surface-container-high text-on-surface-variant'
      } ${statusCls[status]}`}
    >
      {typeLabel[type] ?? type}
    </span>
  );
}
