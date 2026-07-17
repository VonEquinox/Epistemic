import { Link, useParams } from 'react-router-dom';
import { useProjectCoverage } from '../api/hooks';

export function ProjectDetailPage() {
  const { id } = useParams();
  const { data, isLoading } = useProjectCoverage(id);

  return (
    <div className="max-w-3xl mx-auto p-4 md:p-6 space-y-4">
      <Link to="/projects" className="md-btn-text md-btn-sm -ml-3">
        ← 项目列表
      </Link>
      <h1 className="text-xl font-medium text-on-surface">团队覆盖</h1>
      {isLoading && <p className="text-on-surface-variant text-sm">加载…</p>}
      <ul className="md-card-outlined overflow-hidden divide-y divide-outline-variant">
        {data?.map((e) => (
          <li
            key={e.work_id}
            className="p-4 text-sm hover:bg-surface-container-low transition-colors"
          >
            <Link to={`/papers/${e.work_id}`} className="font-medium text-on-surface hover:text-primary">
              {e.title}
            </Link>
            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              {e.readers.length === 0 ? (
                <span className="text-xs text-on-surface-variant">无人阅读</span>
              ) : (
                e.readers.map((r, i) => (
                  <span key={i} className="md-chip-static">
                    {r.name}({r.status})
                  </span>
                ))
              )}
            </div>
          </li>
        ))}
        {data?.length === 0 && (
          <li className="p-4">
            <p className="text-on-surface-variant text-sm">项目下还没有论文。</p>
          </li>
        )}
      </ul>
    </div>
  );
}
