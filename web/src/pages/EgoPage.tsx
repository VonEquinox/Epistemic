import { useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useEgo } from '../api/hooks';
import { EgoView } from '../graph/EgoView';
import { RelationBadge } from '../components/RelationBadge';

export function EgoPage() {
  const { kind = 'work', id } = useParams();
  const [depth, setDepth] = useState(1);
  const { data, isLoading, error } = useEgo(kind, id, depth);
  const [selectedRel, setSelectedRel] = useState<string | null>(null);

  const edge = data?.edges.find((e) => e.relation_id === selectedRel);

  return (
    <div className="h-full flex flex-col">
      <div className="h-11 border-b border-ink-100 bg-white px-4 flex items-center gap-4 text-sm">
        <Link to="/map" className="text-ink-500 hover:text-ink-800">
          ← 地图
        </Link>
        <span className="font-medium">
          Ego · {data?.center.label ?? id}
        </span>
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
      </div>
      <div className="flex-1 min-h-0 flex">
        <div className="flex-1 p-3">
          {isLoading && <p className="text-ink-500 text-sm">加载…</p>}
          {error && (
            <p className="text-rose-600 text-sm">{(error as Error).message}</p>
          )}
          {data && (
            <EgoView data={data} onSelectEdge={(rid) => setSelectedRel(rid)} />
          )}
        </div>
        {edge && (
          <aside className="w-80 border-l border-ink-200 bg-white p-4 text-sm space-y-3 overflow-y-auto">
            <h3 className="font-medium">关系证据</h3>
            <RelationBadge
              type={edge.relation_type}
              status={edge.review_status}
            />
            <p className="text-ink-700">{edge.explanation || '（无解释）'}</p>
            <p className="text-xs text-ink-400">
              {edge.source_layer} · confidence{' '}
              {edge.confidence?.toFixed(2) ?? '—'} · reviews {edge.review_count}
            </p>
            <Link
              to="/review"
              className="text-accent text-xs hover:underline"
            >
              去审核队列处理
            </Link>
          </aside>
        )}
      </div>
    </div>
  );
}
