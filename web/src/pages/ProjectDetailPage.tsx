import { Link, useParams } from 'react-router-dom';
import { useProjectCoverage } from '../api/hooks';

export function ProjectDetailPage() {
  const { id } = useParams();
  const { data, isLoading } = useProjectCoverage(id);

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-4">
      <Link to="/projects" className="text-sm text-ink-500 hover:text-ink-800">
        ← 项目列表
      </Link>
      <h1 className="text-lg font-semibold">团队覆盖</h1>
      {isLoading && <p className="text-ink-500 text-sm">加载…</p>}
      <ul className="space-y-2">
        {data?.map((e) => (
          <li
            key={e.work_id}
            className="border border-ink-200 rounded-lg p-3 bg-white text-sm"
          >
            <Link to={`/papers/${e.work_id}`} className="font-medium hover:text-accent">
              {e.title}
            </Link>
            <div className="mt-1 text-ink-500">
              {e.readers.length === 0
                ? '无人阅读'
                : e.readers.map((r) => `${r.name}(${r.status})`).join(' · ')}
            </div>
          </li>
        ))}
        {data?.length === 0 && (
          <p className="text-ink-400 text-sm">项目下还没有论文。</p>
        )}
      </ul>
    </div>
  );
}
