import { useUiStore } from '../stores/ui';
import { useWork } from '../api/hooks';
import { PaperCard } from './PaperCard';
import { NodeComments } from './NodeComments';
import { Link, useNavigate } from 'react-router-dom';
import { createPortal } from 'react-dom';

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

  return createPortal(
    <aside className="fixed right-0 top-14 bottom-0 z-[100] w-[min(420px,calc(100vw-1rem))] bg-surface-container-lowest border-l border-outline-variant shadow-elev3 rounded-l-2xl flex flex-col">
      <div className="h-12 shrink-0 flex items-center justify-between px-4 border-b border-outline-variant">
        <span className="text-sm font-medium text-on-surface">论文卡片</span>
        <div className="flex items-center gap-2">
          <Link
            to={detailHref}
            className="text-xs text-primary hover:underline"
          >
            全页
          </Link>
          <Link
            to={`/ego/work/${id}`}
            className="text-xs text-primary hover:underline"
          >
            Ego
          </Link>
          <button
            className="md-icon-btn"
            onClick={() => selectWork(null)}
          >
            ×
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-4">
        {isLoading && <p className="text-on-surface-variant text-sm">加载中…</p>}
        {error && (
          <p className="text-error text-sm">{(error as Error).message}</p>
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
    </aside>,
    document.body,
  );
}
