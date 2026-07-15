import { useEffect, useState } from 'react';
import { usePatchRelation, useReviewAction, useReviewQueue } from '../api/hooks';
import { RelationBadge } from '../components/RelationBadge';

export function ReviewPage() {
  const { data, isLoading, refetch } = useReviewQueue();
  const review = useReviewAction();
  const patch = usePatchRelation();
  const [idx, setIdx] = useState(0);

  const items = data ?? [];
  const current = items[idx];

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!current) return;
      if (e.key === 'j') setIdx((i) => Math.min(items.length - 1, i + 1));
      if (e.key === 'k') setIdx((i) => Math.max(0, i - 1));
      if (e.key === 'a') {
        review.mutate(
          { id: current.relation.id, verdict: 'agree' },
          { onSuccess: () => refetch() },
        );
      }
      if (e.key === 'r') {
        review.mutate(
          { id: current.relation.id, verdict: 'disagree' },
          { onSuccess: () => refetch() },
        );
      }
      if (e.key === 'f') {
        patch.mutate(
          { id: current.relation.id, body: { swap_direction: true } },
          { onSuccess: () => refetch() },
        );
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [current, items.length, review, patch, refetch]);

  return (
    <div className="max-w-3xl mx-auto p-6">
      <div className="flex items-baseline justify-between mb-4">
        <h1 className="text-lg font-semibold">审核队列</h1>
        <p className="text-xs text-ink-400">
          键盘：j/k 移动 · a 接受 · r 拒绝 · f 调转方向
        </p>
      </div>

      {isLoading && <p className="text-ink-500 text-sm">加载…</p>}
      {!isLoading && items.length === 0 && (
        <p className="text-ink-500 text-sm">队列为空。AI 候选关系就绪后会出现在这里。</p>
      )}

      <div className="space-y-2">
        {items.map((item, i) => {
          const r = item.relation;
          const active = i === idx;
          return (
            <div
              key={r.id}
              onClick={() => setIdx(i)}
              className={`border rounded-lg p-4 cursor-pointer ${
                active
                  ? 'border-accent bg-accent-soft/40'
                  : 'border-ink-200 bg-white'
              }`}
            >
              <div className="flex items-center gap-2 mb-2">
                <RelationBadge type={r.type} status={r.review_status} />
                {r.confidence != null && (
                  <span className="text-xs text-ink-400">
                    conf {r.confidence.toFixed(2)}
                  </span>
                )}
              </div>
              <p className="text-sm text-ink-800">{r.explanation || '（无解释）'}</p>
              {item.evidence.length > 0 && (
                <blockquote className="mt-2 text-xs text-ink-600 border-l-2 border-ink-200 pl-3 italic">
                  p.{item.evidence[0].page}: “{item.evidence[0].text}”
                  {item.members.find((m) => m.role === 'source')?.anchor_work_id && (
                    <a
                      className="ml-2 not-italic text-accent hover:underline"
                      href={`/papers/${item.members.find((m) => m.role === 'source')?.anchor_work_id}?page=${item.evidence[0].page}&evidence=${item.evidence[0].id}`}
                    >
                      跳到 PDF
                    </a>
                  )}
                </blockquote>
              )}
              {active && (
                <div className="mt-3 flex gap-2">
                  <button
                    className="px-3 py-1 text-xs rounded bg-emerald-600 text-white"
                    onClick={() =>
                      review.mutate(
                        { id: r.id, verdict: 'agree' },
                        { onSuccess: () => refetch() },
                      )
                    }
                  >
                    接受 (a)
                  </button>
                  <button
                    className="px-3 py-1 text-xs rounded bg-rose-600 text-white"
                    onClick={() =>
                      review.mutate(
                        { id: r.id, verdict: 'disagree' },
                        { onSuccess: () => refetch() },
                      )
                    }
                  >
                    拒绝 (r)
                  </button>
                  <button
                    className="px-3 py-1 text-xs rounded border border-ink-200"
                    onClick={() =>
                      patch.mutate(
                        { id: r.id, body: { swap_direction: true } },
                        { onSuccess: () => refetch() },
                      )
                    }
                  >
                    调转方向 (f)
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
