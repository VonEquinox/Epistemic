import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import { usePatchRelation, useReviewAction, useReviewQueue } from '../api/hooks';
import { RelationBadge } from '../components/RelationBadge';
import type { RelationDetail, RelationType } from '../api/types';

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

/** 最近一次可撤销的审核决策 */
type UndoEntry = {
  item: RelationDetail;
  /** 撤销前的索引，尽量还原光标位置 */
  index: number;
  verdict: 'agree' | 'disagree';
};

const MAX_UNDO = 20;

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  return el.isContentEditable;
}

export function ReviewPage() {
  const qc = useQueryClient();
  const { data, isLoading } = useReviewQueue();
  const review = useReviewAction();
  const patch = usePatchRelation();
  const [idx, setIdx] = useState(0);
  const [editingType, setEditingType] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [rejecting, setRejecting] = useState(false);
  const [rejectComment, setRejectComment] = useState('');
  const [undoStack, setUndoStack] = useState<UndoEntry[]>([]);
  const [actionError, setActionError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const rejectInputRef = useRef<HTMLInputElement>(null);

  const items = data ?? [];
  const current = items[idx];

  // 索引越界夹紧
  useEffect(() => {
    if (idx >= items.length && items.length > 0) {
      setIdx(items.length - 1);
    }
  }, [items.length, idx]);

  // 切换条目时收起展开 / 拒绝表单
  useEffect(() => {
    setExpanded(false);
    setEditingType(false);
    setRejecting(false);
    setRejectComment('');
  }, [current?.relation.id]);

  useEffect(() => {
    if (rejecting) rejectInputRef.current?.focus();
  }, [rejecting]);

  // toast 自动消失
  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), 2200);
    return () => window.clearTimeout(t);
  }, [toast]);

  const pushUndo = useCallback((entry: UndoEntry) => {
    setUndoStack((s) => [...s.slice(-(MAX_UNDO - 1)), entry]);
  }, []);

  const doAccept = useCallback(
    (item: RelationDetail, at: number) => {
      setActionError(null);
      pushUndo({ item, index: at, verdict: 'agree' });
      review.mutate(
        { id: item.relation.id, verdict: 'agree' },
        {
          onError: (err) => {
            setActionError(err instanceof Error ? err.message : '接受失败');
            setUndoStack((s) => s.filter((e) => e.item.relation.id !== item.relation.id));
          },
        },
      );
    },
    [pushUndo, review],
  );

  const doReject = useCallback(
    (item: RelationDetail, at: number, comment?: string) => {
      setActionError(null);
      setRejecting(false);
      setRejectComment('');
      pushUndo({ item, index: at, verdict: 'disagree' });
      review.mutate(
        {
          id: item.relation.id,
          verdict: 'disagree',
          comment: comment?.trim() || undefined,
        },
        {
          onError: (err) => {
            setActionError(err instanceof Error ? err.message : '拒绝失败');
            setUndoStack((s) => s.filter((e) => e.item.relation.id !== item.relation.id));
          },
        },
      );
    },
    [pushUndo, review],
  );

  const doUndo = useCallback(() => {
    const last = undoStack[undoStack.length - 1];
    if (!last) {
      setToast('没有可撤销的操作');
      return;
    }
    setUndoStack((s) => s.slice(0, -1));
    setActionError(null);

    // 乐观：把快照塞回队列，并定位光标
    const restored: RelationDetail = {
      ...last.item,
      relation: { ...last.item.relation, review_status: 'unreviewed' },
    };
    const prev = qc.getQueryData<RelationDetail[]>(['review-queue']) ?? [];
    if (!prev.some((x) => x.relation.id === restored.relation.id)) {
      const insertAt = Math.min(last.index, prev.length);
      const next = [...prev];
      next.splice(insertAt, 0, restored);
      qc.setQueryData(['review-queue'], next);
    }
    setIdx(Math.min(last.index, Math.max(0, prev.length)));
    setToast(last.verdict === 'agree' ? '已撤销接受' : '已撤销拒绝');

    // 服务端撤销：PATCH review_status=unreviewed（会清掉 reviews 行）
    patch.mutate(
      {
        id: last.item.relation.id,
        body: { review_status: 'unreviewed' },
      },
      {
        onError: (err) => {
          setActionError(err instanceof Error ? err.message : '撤销失败');
          qc.setQueryData<RelationDetail[]>(['review-queue'], (cur) =>
            (cur ?? []).filter((x) => x.relation.id !== restored.relation.id),
          );
          setUndoStack((s) => [...s, last]);
        },
      },
    );
  }, [patch, qc, undoStack]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // 拒绝理由输入框：Enter/r 提交 · Esc 取消（输入时不拦截 j/k 以外的全局键）
      if (rejecting) {
        if (e.key === 'Escape') {
          e.preventDefault();
          setRejecting(false);
          setRejectComment('');
          return;
        }
        if ((e.key === 'Enter' || e.key === 'r') && current) {
          // 在 input 里按 r 应作为字符输入，仅 Enter / 非输入态 r 确认
          if (e.key === 'r' && isTypingTarget(e.target)) return;
          e.preventDefault();
          doReject(current, idx, rejectComment);
          return;
        }
        if (isTypingTarget(e.target)) return;
      }

      if (isTypingTarget(e.target)) return;
      if (!current && e.key !== 'u') return;

      if (e.key === 'j') {
        e.preventDefault();
        setIdx((i) => Math.min(items.length - 1, i + 1));
      }
      if (e.key === 'k') {
        e.preventDefault();
        setIdx((i) => Math.max(0, i - 1));
      }
      if (e.key === 'a' && current) {
        e.preventDefault();
        doAccept(current, idx);
      }
      if (e.key === 'r' && current) {
        e.preventDefault();
        // 打开拒绝理由（可选）；理由面板已开时 r 直接拒绝
        if (rejecting) {
          doReject(current, idx, rejectComment);
        } else {
          setRejecting(true);
        }
      }
      if (e.key === 'f' && current) {
        e.preventDefault();
        setActionError(null);
        patch.mutate(
          { id: current.relation.id, body: { swap_direction: true } },
          {
            onError: (err) =>
              setActionError(err instanceof Error ? err.message : '调转失败'),
          },
        );
      }
      if (e.key === 'e' && current) {
        e.preventDefault();
        setEditingType((v) => !v);
      }
      if (e.key === 'u') {
        e.preventDefault();
        doUndo();
      }
      if (e.key === 'Enter' && current) {
        e.preventDefault();
        setExpanded((v) => !v);
      }
      if (e.key === 'Escape') {
        setEditingType(false);
        setExpanded(false);
        setRejecting(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [
    current,
    idx,
    items.length,
    rejecting,
    rejectComment,
    doAccept,
    doReject,
    doUndo,
    patch,
  ]);

  return (
    <div className="max-w-3xl mx-auto p-6">
      <div className="flex items-baseline justify-between mb-4 gap-3 flex-wrap">
        <h1 className="text-lg font-semibold">
          审核队列
          {items.length > 0 && (
            <span className="ml-2 text-sm font-normal text-ink-400">
              {items.length} 条候选
            </span>
          )}
        </h1>
        <p className="text-xs text-ink-400">
          键盘：j/k 移动 · a 接受 · r 拒绝 · f 调转 · e 改类型 · Enter 展开证据 · u 撤销
        </p>
      </div>

      {toast && (
        <div className="mb-3 text-xs px-3 py-2 rounded bg-ink-800 text-white inline-block">
          {toast}
          {undoStack.length > 0 && (
            <span className="ml-2 opacity-70">（还可撤销 {undoStack.length} 步）</span>
          )}
        </div>
      )}

      {actionError && (
        <div className="mb-3 text-xs px-3 py-2 rounded bg-rose-50 text-rose-700 border border-rose-200">
          {actionError}
          <button
            className="ml-2 underline"
            onClick={() => setActionError(null)}
            type="button"
          >
            关闭
          </button>
        </div>
      )}

      {isLoading && <p className="text-ink-500 text-sm">加载…</p>}
      {!isLoading && items.length === 0 && (
        <p className="text-ink-500 text-sm">
          队列为空。AI 候选关系就绪后会出现在这里。
          {undoStack.length > 0 && (
            <button
              type="button"
              className="ml-2 text-accent hover:underline"
              onClick={doUndo}
            >
              撤销上一步 (u)
            </button>
          )}
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
          const showAllEvidence = active && expanded;
          const evidenceList = showAllEvidence
            ? item.evidence
            : item.evidence.slice(0, 1);

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
                {item.evidence.length > 1 && (
                  <span className="text-xs text-ink-400">
                    {item.evidence.length} 条证据
                    {active ? (expanded ? ' · Enter 收起' : ' · Enter 展开') : ''}
                  </span>
                )}
              </div>
              <p className="text-sm text-ink-800">{r.explanation || '（无解释）'}</p>

              <p className="mt-1 text-xs text-ink-500">
                {item.members
                  .filter((m) => m.role === 'source' || m.role === 'target')
                  .sort((a, b) => (a.role === 'source' ? -1 : 1))
                  .map((m) => `${m.role}: ${m.entity_id.slice(0, 8)}…`)
                  .join('  →  ')}
              </p>

              {evidenceList.map((ev) => (
                <blockquote
                  key={ev.id}
                  className="mt-2 text-xs text-ink-600 border-l-2 border-ink-200 pl-3 italic"
                >
                  p.{ev.page}: “{ev.text}”
                  {item.members.find((m) => m.role === 'source')?.anchor_work_id && (
                    <a
                      className="ml-2 not-italic text-accent hover:underline"
                      href={`/papers/${
                        item.members.find((m) => m.role === 'source')?.anchor_work_id
                      }?page=${ev.page}&evidence=${ev.id}`}
                      onClick={(e) => e.stopPropagation()}
                    >
                      跳到 PDF
                    </a>
                  )}
                </blockquote>
              ))}

              {active && (
                <div className="mt-3 flex flex-wrap gap-2 items-center">
                  <button
                    type="button"
                    className="px-3 py-1 text-xs rounded bg-emerald-600 text-white"
                    onClick={(e) => {
                      e.stopPropagation();
                      doAccept(item, i);
                    }}
                  >
                    接受 (a)
                  </button>
                  <button
                    type="button"
                    className="px-3 py-1 text-xs rounded bg-rose-600 text-white"
                    onClick={(e) => {
                      e.stopPropagation();
                      setRejecting((v) => !v);
                    }}
                  >
                    拒绝 (r)
                  </button>
                  <button
                    type="button"
                    className="px-3 py-1 text-xs rounded border border-ink-200"
                    onClick={(e) => {
                      e.stopPropagation();
                      setActionError(null);
                      patch.mutate(
                        { id: r.id, body: { swap_direction: true } },
                        {
                          onError: (err) =>
                            setActionError(
                              err instanceof Error ? err.message : '调转失败',
                            ),
                        },
                      );
                    }}
                  >
                    调转方向 (f)
                  </button>
                  <button
                    type="button"
                    className="px-3 py-1 text-xs rounded border border-ink-200"
                    onClick={(e) => {
                      e.stopPropagation();
                      setEditingType((v) => !v);
                    }}
                  >
                    改类型 (e)
                  </button>
                  {item.evidence.length > 0 && (
                    <button
                      type="button"
                      className="px-3 py-1 text-xs rounded border border-ink-200"
                      onClick={(e) => {
                        e.stopPropagation();
                        setExpanded((v) => !v);
                      }}
                    >
                      {expanded ? '收起证据' : '展开证据'} (Enter)
                    </button>
                  )}
                  {undoStack.length > 0 && (
                    <button
                      type="button"
                      className="px-3 py-1 text-xs rounded border border-ink-200 text-ink-500"
                      onClick={(e) => {
                        e.stopPropagation();
                        doUndo();
                      }}
                    >
                      撤销 (u)
                    </button>
                  )}

                  {editingType && (
                    <select
                      className="text-xs border border-ink-200 rounded px-2 py-1"
                      defaultValue={r.type}
                      autoFocus
                      onClick={(e) => e.stopPropagation()}
                      onChange={(ev) => {
                        const next = ev.target.value as RelationType;
                        setActionError(null);
                        patch.mutate(
                          { id: r.id, body: { relation_type: next } },
                          {
                            onSuccess: () => setEditingType(false),
                            onError: (err) =>
                              setActionError(
                                err instanceof Error ? err.message : '改类型失败',
                              ),
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

              {active && rejecting && (
                <div
                  className="mt-3 flex flex-wrap gap-2 items-center"
                  onClick={(e) => e.stopPropagation()}
                >
                  <input
                    ref={rejectInputRef}
                    type="text"
                    className="flex-1 min-w-[12rem] text-xs border border-ink-200 rounded px-2 py-1.5"
                    placeholder="拒绝理由（可选）"
                    value={rejectComment}
                    onChange={(ev) => setRejectComment(ev.target.value)}
                  />
                  <button
                    type="button"
                    className="px-3 py-1 text-xs rounded bg-rose-600 text-white"
                    onClick={() => doReject(item, i, rejectComment)}
                  >
                    确认拒绝
                  </button>
                  <button
                    type="button"
                    className="px-3 py-1 text-xs rounded border border-ink-200"
                    onClick={() => {
                      setRejecting(false);
                      setRejectComment('');
                    }}
                  >
                    取消
                  </button>
                  <span className="text-[11px] text-ink-400">
                    Enter 确认 · Esc 取消 · 可留空
                  </span>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
