import type { ReadingLevel } from '../api/types';

const colors: Record<ReadingLevel, string> = {
  unread: 'bg-ink-300',
  skimmed: 'bg-amber-400',
  read: 'bg-emerald-500',
  reproduced: 'bg-blue-600',
  needs_review: 'bg-rose-500',
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
    <span className="inline-flex items-center gap-1.5 text-xs text-ink-600">
      <span className={`w-2 h-2 rounded-full ${colors[status]}`} />
      {labels[status]}
    </span>
  );
}
