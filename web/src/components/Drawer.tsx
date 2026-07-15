import { useUiStore } from '../stores/ui';
import { useWork } from '../api/hooks';
import { PaperCard } from './PaperCard';
import { Link } from 'react-router-dom';

export function Drawer() {
  const id = useUiStore((s) => s.selectedWorkId);
  const open = useUiStore((s) => s.drawerOpen);
  const selectWork = useUiStore((s) => s.selectWork);
  const { data, isLoading, error } = useWork(id ?? undefined);

  if (!open || !id) return null;

  return (
    <aside className="absolute right-0 top-0 bottom-0 w-[420px] bg-white border-l border-ink-200 shadow-xl z-20 flex flex-col">
      <div className="h-12 flex items-center justify-between px-4 border-b border-ink-100">
        <span className="text-sm font-medium text-ink-700">论文卡片</span>
        <div className="flex gap-2">
          <Link
            to={`/papers/${id}`}
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
        {data && <PaperCard card={data} />}
      </div>
    </aside>
  );
}
