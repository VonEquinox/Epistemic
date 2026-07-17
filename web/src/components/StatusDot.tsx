import type { ReadingLevel } from '../api/types';

const colors: Record<ReadingLevel, string> = {
  unread: 'bg-outline-variant',
  skimmed: 'bg-outline',
  read: 'bg-primary',
  reproduced: 'bg-tertiary',
  needs_review: 'bg-error',
};

const labels: Record<ReadingLevel, string> = {
  unread: '未读',
  skimmed: '略读',
  read: '精读',
  reproduced: '复现过',
  needs_review: '需复核',
};

export function StatusDot({ status }: { status: ReadingLevel }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs text-on-surface-variant">
      <span className={`w-2 h-2 rounded-full ${colors[status]}`} />
      {labels[status]}
    </span>
  );
}
