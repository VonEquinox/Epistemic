import { useUiStore } from '../stores/ui';
import { useWork } from '../api/hooks';
import { PaperCard } from './PaperCard';
import { NodeComments } from './NodeComments';
import { Link, useNavigate } from 'react-router-dom';

export function Drawer() {
  const id = useUiStore((s) => s.selectedWorkId);
  const open = useUiStore((s) => s.drawerOpen);
  const selectWork = useUiStore((s) => s.selectWork);
  const graphId = useUiStore((s) => s.activeGraphId);
  const detailHref = graphId
    ? `/papers/${id}?graph=${encodeURIComponent(graphId)}`
    : `/papers/${id}`;
  const { data, isLoading, error } = useWork(id ?? undefined);
  const navigate = useNavigate();

  if (!open || !id) return null;

  return (
    <aside className="absolute right-0 top-0 bottom-0 w-[420px] bg-white border-l border-ink-200 shadow-xl z-20 flex flex-col">
      <div className="h-12 flex items-center justify-between px-4 border-b border-ink-100">
        <span className="text-sm font-medium text-ink-700">论文卡片</span>
        <div className="flex gap-2">
          <Link
            to={detailHref}
            className="text-xs text-accent hover:underline"
          >
            全页
          </Link>
          <Link
            to={`/ego/work/${id}`}
            className="text-xs text-accent hover:underline"
          >
            Ego
          </Link>
          <button
            className="text-ink-400 hover:text-ink-800 text-sm"
            onClick={() => selectWork(null)}
          >
            ✕
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-4">
        {isLoading && <p className="text-ink-500 text-sm">加载中…</p>}
        {error && (
          <p className="text-rose-600 text-sm">{(error as Error).message}</p>
        )}
        {data && (
          <>
            <PaperCard
              card={data}
              onJumpEvidence={(ev) => {
                // Drawer has no PDF pane — open full paper detail at evidence.
                const q = new URLSearchParams();
                if (graphId) q.set('graph', graphId);
                q.set('evidence', ev.id);
                if (ev.page) q.set('page', String(ev.page));
                navigate(`/papers/${id}?${q.toString()}`);
              }}
            />
            <NodeComments graphId={graphId} workId={id} />
          </>
        )}
      </div>
    </aside>
  );
}
