import { useMemo, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { useEgo, useReviewAction } from '../api/hooks';
import { api } from '../api/client';
import type { EgoResponse, RelationDetail } from '../api/types';
import { EgoView } from '../graph/EgoView';
import { RelationBadge } from '../components/RelationBadge';

type Mode = 'explore' | 'review' | 'write';

export function EgoPage() {
  const { kind = 'work', id } = useParams();
  const [depth, setDepth] = useState(1);
  const [mode, setMode] = useState<Mode>('explore');
  const { data: raw, isLoading, error, refetch } = useEgo(kind, id, depth, mode);
  const [selectedRels, setSelectedRels] = useState<string[]>([]);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const review = useReviewAction();

  const data = useMemo(() => {
    if (!raw) return undefined;
    return expandGroups(raw, expandedGroups);
  }, [raw, expandedGroups]);

  const primaryRel = selectedRels[0] ?? null;
  const edge = data?.edges.find((e) => e.relation_id === primaryRel);
  const { data: detail } = useQuery({
    queryKey: ['relation', primaryRel],
    queryFn: () => api.get<RelationDetail>(`/relations/${primaryRel}`),
    enabled: !!primaryRel,
  });

  const onSelectNode = (nid: string, nkind: string, groupKey?: string) => {
    if (nkind === 'group' && groupKey) {
      setExpandedGroups((prev) => {
        const next = new Set(prev);
        if (next.has(groupKey)) next.delete(groupKey);
        else next.add(groupKey);
        return next;
      });
      setSelectedRels([]);
      return;
    }
    // clicking a work node is a no-op for now (could navigate)
  };

  return (
    <div className="h-full flex flex-col">
      <div className="h-11 border-b border-ink-100 bg-white px-4 flex items-center gap-4 text-sm">
        <Link to="/map" className="text-ink-500 hover:text-ink-800">
          ← 地图
        </Link>
        <span className="font-medium">Ego · {data?.center.label ?? id}</span>
        <label className="text-xs text-ink-600 flex items-center gap-1">
          跳数
          <select
            value={depth}
            onChange={(e) => setDepth(Number(e.target.value))}
            className="border border-ink-200 rounded px-1"
          >
            <option value={1}>1</option>
            <option value={2}>2</option>
          </select>
        </label>
        <label className="text-xs text-ink-600 flex items-center gap-1">
          模式
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value as Mode)}
            className="border border-ink-200 rounded px-1"
          >
            <option value="explore">探索</option>
            <option value="review">审读</option>
            <option value="write">写作</option>
          </select>
        </label>
        {data?.groups && data.groups.length > 0 && (
          <span className="text-xs text-ink-400">
            溢出组 {data.groups.length}
            {expandedGroups.size > 0 && ` · 已展开 ${expandedGroups.size}`}
          </span>
        )}
      </div>
      <div className="flex-1 min-h-0 flex">
        <div className="flex-1 p-3">
          {isLoading && <p className="text-ink-500 text-sm">加载…</p>}
          {error && (
            <p className="text-rose-600 text-sm">{(error as Error).message}</p>
          )}
          {data && (
            <EgoView
              data={data}
              onSelectBundle={(ids) => setSelectedRels(ids)}
              onSelectNode={onSelectNode}
            />
          )}
        </div>
        {(selectedRels.length > 0 || edge) && (
          <aside className="w-80 border-l border-ink-200 bg-white p-4 text-sm space-y-3 overflow-y-auto">
            <h3 className="font-medium">
              关系证据
              {selectedRels.length > 1 && (
                <span className="ml-2 text-xs font-normal text-ink-400">
                  束内 {selectedRels.length} 条
                </span>
              )}
            </h3>

            {selectedRels.length > 1 && (
              <ul className="space-y-1 max-h-32 overflow-y-auto">
                {selectedRels.map((rid) => {
                  const e = data?.edges.find((x) => x.relation_id === rid);
                  if (!e) return null;
                  return (
                    <li key={rid}>
                      <button
                        type="button"
                        className={`w-full text-left text-xs px-2 py-1 rounded border ${
                          rid === primaryRel
                            ? 'border-accent bg-blue-50'
                            : 'border-ink-100 hover:bg-ink-50'
                        }`}
                        onClick={() =>
                          setSelectedRels([rid, ...selectedRels.filter((x) => x !== rid)])
                        }
                      >
                        <RelationBadge type={e.relation_type} status={e.review_status} />
                        <span className="ml-1 text-ink-500">
                          {e.confidence?.toFixed(2) ?? '—'}
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}

            {edge && (
              <>
                <RelationBadge
                  type={edge.relation_type}
                  status={edge.review_status}
                />
                <p className="text-ink-700">{edge.explanation || '（无解释）'}</p>
                <p className="text-xs text-ink-400">
                  {edge.source_layer} · confidence{' '}
                  {edge.confidence?.toFixed(2) ?? '—'} · reviews {edge.review_count}
                </p>

                {detail?.evidence?.map((ev) => (
                  <blockquote
                    key={ev.id}
                    className="text-xs text-ink-600 border-l-2 border-ink-200 pl-3 italic"
                  >
                    p.{ev.page}: “{ev.text}”
                    {detail.members.find((m) => m.role === 'source')
                      ?.anchor_work_id && (
                      <a
                        className="ml-2 not-italic text-accent hover:underline"
                        href={`/papers/${
                          detail.members.find((m) => m.role === 'source')
                            ?.anchor_work_id
                        }?page=${ev.page}&evidence=${ev.id}`}
                      >
                        跳到 PDF
                      </a>
                    )}
                  </blockquote>
                ))}

                <div className="flex gap-2 pt-2">
                  <button
                    className="px-3 py-1 text-xs rounded bg-emerald-600 text-white"
                    onClick={() =>
                      review.mutate(
                        { id: edge.relation_id, verdict: 'agree' },
                        {
                          onSuccess: () => {
                            refetch();
                            setSelectedRels([]);
                          },
                        },
                      )
                    }
                  >
                    确认
                  </button>
                  <button
                    className="px-3 py-1 text-xs rounded bg-rose-600 text-white"
                    onClick={() =>
                      review.mutate(
                        { id: edge.relation_id, verdict: 'disagree' },
                        {
                          onSuccess: () => {
                            refetch();
                            setSelectedRels([]);
                          },
                        },
                      )
                    }
                  >
                    反对
                  </button>
                </div>
              </>
            )}

            <Link to="/review" className="text-accent text-xs hover:underline">
              去审核队列处理
            </Link>
            {data?.center.work_id && (
              <Link
                to={`/papers/${data.center.work_id}`}
                className="block text-accent text-xs hover:underline"
              >
                打开论文 PDF
              </Link>
            )}
          </aside>
        )}
      </div>
    </div>
  );
}

/** Expand overflow group nodes into member work nodes (client-side). */
function expandGroups(raw: EgoResponse, expanded: Set<string>): EgoResponse {
  if (!raw.groups?.length || expanded.size === 0) return raw;

  const groupNodes = raw.nodes.filter((n) => n.kind === 'group');
  const keepNodes = raw.nodes.filter(
    (n) => n.kind !== 'group' || !n.group_key || !expanded.has(n.group_key),
  );
  const edges = raw.edges.filter((e) => {
    // drop edges that only connect collapsed group nodes we're expanding
    const isGroupEdge =
      groupNodes.some((g) => g.id === e.source_id || g.id === e.target_id) &&
      groupNodes.some((g) => expanded.has(g.group_key ?? ''));
    if (!isGroupEdge) return true;
    // drop if either endpoint is an expanded group
    const srcG = groupNodes.find((g) => g.id === e.source_id);
    const tgtG = groupNodes.find((g) => g.id === e.target_id);
    if (srcG?.group_key && expanded.has(srcG.group_key)) return false;
    if (tgtG?.group_key && expanded.has(tgtG.group_key)) return false;
    return true;
  });

  const newNodes = [...keepNodes];
  const newEdges = [...edges];
  const centerId = raw.center.id;

  for (const g of raw.groups) {
    if (!expanded.has(g.key)) continue;
    const groupNode = groupNodes.find((n) => n.group_key === g.key);
    // members may not have been in the original selection; invent stubs from ids
    for (const mid of g.member_work_ids) {
      if (newNodes.some((n) => n.id === mid || n.work_id === mid)) continue;
      newNodes.push({
        id: mid,
        kind: 'work',
        label: mid.slice(0, 8) + '…',
        work_id: mid,
        score: 0,
      });
      // synthetic edge center→member so layout has connectivity
      newEdges.push({
        relation_id: `expand-${g.key}-${mid}`,
        source_id: centerId,
        target_id: mid,
        relation_type: g.relation_type,
        review_status: 'unreviewed',
        source_layer: 'ai_candidate',
        confidence: null,
        explanation: `展开自组 ${g.key}`,
        review_count: 0,
        bundle_key: null,
      });
    }
    // remove group node already filtered in keepNodes
    void groupNode;
  }

  return { ...raw, nodes: newNodes, edges: newEdges };
}
