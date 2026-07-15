import { useEffect, useState } from 'react';
import { usePatchRelation, useReviewAction, useReviewQueue } from '../api/hooks';
import { RelationBadge } from '../components/RelationBadge';
import type { RelationType } from '../api/types';

const RELATION_TYPES: RelationType[] = [
  'uses_method_from',
  'improves_on',
  'alternative_to',
  'uses_dataset_from',
  'compares_against',
  'reproduces',
  'fails_to_reproduce',
  'supports_claim',
  'contradicts_claim',
  'prerequisite_for',
];

export function ReviewPage() {
  const { data, isLoading, refetch } = useReviewQueue();
  const review = useReviewAction();
  const patch = usePatchRelation();
  const [idx, setIdx] = useState(0);
  const [editingType, setEditingType] = useState(false);

  const items = data ?? [];
  const current = items[idx];

  useEffect(() => {
    // Clamp index when queue shrinks after accept/reject
    if (idx >= items.length && items.length > 0) {
      setIdx(items.length - 1);
    }
  }, [items.length, idx]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!current) return;
      // Ignore when typing in select
      if ((e.target as HTMLElement)?.tagName === 'SELECT') return;

      if (e.key === 'j') setIdx((i) => Math.min(items.length - 1, i + 1));
      if (e.key === 'k') setIdx((i) => Math.max(0, i - 1));
      if (e.key === 'a') {
        review.mutate(
          { id: current.relation.id, verdict: 'agree' },
          {
            onSuccess: () => {
              refetch();
              setIdx((i) => Math.min(i, Math.max(0, items.length - 2)));
            },
          },
        );
      }
      if (e.key === 'r') {
        // Disagree → rejected when sole reviewer (backend recompute_status)
        review.mutate(
          { id: current.relation.id, verdict: 'disagree' },
          {
            onSuccess: () => {
              refetch();
              setIdx((i) => Math.min(i, Math.max(0, items.length - 2)));
            },
          },
        );
      }
      if (e.key === 'f') {
        patch.mutate(
          { id: current.relation.id, body: { swap_direction: true } },
          { onSuccess: () => refetch() },
        );
      }
      if (e.key === 'e') {
        setEditingType((v) => !v);
      }
      if (e.key === 'Escape') {
        setEditingType(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [current, items.length, review, patch, refetch]);

  return (
    <div className="max-w-3xl mx-auto p-6">
      <div className="flex items-baseline justify-between mb-4">
        <h1 className="text-lg font-semibold">
          审核队列
          {items.length > 0 && (
            <span className="ml-2 text-sm font-normal text-ink-400">
              {items.length} 条候选
            </span>
          )}
        </h1>
        <p className="text-xs text-ink-400">
          键盘：j/k 移动 · a 接受 · r 拒绝 · f 调转 · e 改类型
        </p>
      </div>

      {isLoading && <p className="text-ink-500 text-sm">加载…</p>}
      {!isLoading && items.length === 0 && (
        <p className="text-ink-500 text-sm">
          队列为空。AI 候选关系就绪后会出现在这里。
        </p>
      )}

      <div className="space-y-2">
        {items.map((item, i) => {
          const r = item.relation;
          const active = i === idx;
          const conf = r.confidence;
          const confBand =
            conf == null
              ? null
              : conf >= 0.75
                ? 'high'
                : conf >= 0.5
                  ? 'mid'
                  : 'low';
          return (
            <div
              key={r.id}
              onClick={() => {
                setIdx(i);
                setEditingType(false);
              }}
              className={`border rounded-lg p-4 cursor-pointer ${
                active
                  ? 'border-accent bg-accent-soft/40'
                  : 'border-ink-200 bg-white'
              }`}
            >
              <div className="flex items-center gap-2 mb-2 flex-wrap">
                <RelationBadge type={r.type} status={r.review_status} />
                {r.aspect && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-ink-100 text-ink-600">
                    aspect: {r.aspect}
                  </span>
                )}
                {conf != null && (
                  <span
                    className={`text-xs px-1.5 py-0.5 rounded ${
                      confBand === 'high'
                        ? 'bg-emerald-50 text-emerald-700'
                        : confBand === 'mid'
                          ? 'bg-amber-50 text-amber-700'
                          : 'bg-ink-100 text-ink-500'
                    }`}
                  >
                    conf {conf.toFixed(2)}
                    {confBand === 'mid' ? ' · 较弱' : ''}
                  </span>
                )}
                {r.source === 'ai_candidate' && (
                  <span className="text-xs text-ink-400">AI 候选</span>
                )}
              </div>
              <p className="text-sm text-ink-800">{r.explanation || '（无解释）'}</p>

              {/* Members direction */}
              <p className="mt-1 text-xs text-ink-500">
                {item.members
                  .filter((m) => m.role === 'source' || m.role === 'target')
                  .sort((a, b) => (a.role === 'source' ? -1 : 1))
                  .map((m) => `${m.role}: ${m.entity_id.slice(0, 8)}…`)
                  .join('  →  ')}
              </p>

              {item.evidence.length > 0 && (
                <blockquote className="mt-2 text-xs text-ink-600 border-l-2 border-ink-200 pl-3 italic">
                  p.{item.evidence[0].page}: “{item.evidence[0].text}”
                  {item.members.find((m) => m.role === 'source')?.anchor_work_id && (
                    <a
                      className="ml-2 not-italic text-accent hover:underline"
                      href={`/papers/${
                        item.members.find((m) => m.role === 'source')
                          ?.anchor_work_id
                      }?page=${item.evidence[0].page}&evidence=${item.evidence[0].id}`}
                    >
                      跳到 PDF
                    </a>
                  )}
                </blockquote>
              )}

              {active && (
                <div className="mt-3 flex flex-wrap gap-2 items-center">
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
                  <button
                    className="px-3 py-1 text-xs rounded border border-ink-200"
                    onClick={() => setEditingType((v) => !v)}
                  >
                    改类型 (e)
                  </button>
                  {editingType && (
                    <select
                      className="text-xs border border-ink-200 rounded px-2 py-1"
                      defaultValue={r.type}
                      autoFocus
                      onChange={(ev) => {
                        const next = ev.target.value as RelationType;
                        patch.mutate(
                          { id: r.id, body: { relation_type: next } },
                          {
                            onSuccess: () => {
                              setEditingType(false);
                              refetch();
                            },
                          },
                        );
                      }}
                    >
                      {RELATION_TYPES.map((t) => (
                        <option key={t} value={t}>
                          {t}
                        </option>
                      ))}
                    </select>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
