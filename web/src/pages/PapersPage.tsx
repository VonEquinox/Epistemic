import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuickAdd, useWorks } from '../api/hooks';

export function PapersPage() {
  const [q, setQ] = useState('');
  const [input, setInput] = useState('');
  const { data, isLoading } = useWorks({ query: q || undefined });
  const quickAdd = useQuickAdd();

  return (
    <div className="max-w-5xl mx-auto p-4 md:p-6 space-y-4">
      <h1 className="text-xl font-medium text-on-surface">论文</h1>
      <div className="flex gap-3 items-end">
        <label className="flex-1 text-sm">
          <span className="text-on-surface-variant">搜索</span>
          <input
            className="md-field mt-1 w-full"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="标题 / arXiv id"
          />
        </label>
        <form
          className="flex gap-2 flex-1"
          onSubmit={(e) => {
            e.preventDefault();
            if (!input.trim()) return;
            quickAdd.mutate(input.trim(), { onSuccess: () => setInput('') });
          }}
        >
          <input
            className="md-field flex-1"
            placeholder="快速添加：arXiv URL / DOI"
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
          <button
            type="submit"
            className="md-btn-filled"
          >
            添加
          </button>
        </form>
      </div>

      {quickAdd.isError && (
        <p className="text-sm text-error">{(quickAdd.error as Error).message}</p>
      )}

      <div className="md-card-outlined overflow-hidden">
        <table className="w-full text-sm">
          <thead className="text-on-surface-variant text-xs uppercase tracking-wide text-left">
            <tr>
              <th className="px-3 py-2.5 font-medium">标题</th>
              <th className="px-3 py-2.5 font-medium w-20">年份</th>
              <th className="px-3 py-2.5 font-medium">作者</th>
              <th className="px-3 py-2.5 font-medium w-32">arXiv</th>
            </tr>
          </thead>
          <tbody>
            {isLoading && (
              <tr>
                <td colSpan={4} className="px-3 py-6 text-on-surface-variant border-t border-outline-variant">
                  加载中…
                </td>
              </tr>
            )}
            {data?.map((item) => (
              <tr
                key={item.work.id}
                className="border-t border-outline-variant hover:bg-surface-container-low transition-colors"
              >
                <td className="px-3 py-2">
                  <Link
                    to={`/papers/${item.work.id}`}
                    className="text-on-surface hover:text-primary"
                  >
                    {item.title}
                  </Link>
                </td>
                <td className="px-3 py-2 text-on-surface-variant">{item.year ?? '—'}</td>
                <td className="px-3 py-2 text-on-surface-variant truncate max-w-[200px]">
                  {item.authors.slice(0, 3).join(', ')}
                  {item.authors.length > 3 && ' et al.'}
                </td>
                <td className="px-3 py-2 font-mono text-xs text-on-surface-variant">
                  {item.arxiv_id ?? '—'}
                </td>
              </tr>
            ))}
            {data?.length === 0 && (
              <tr>
                <td colSpan={4} className="px-3 py-6 text-on-surface-variant border-t border-outline-variant">
                  暂无论文。用上方输入框添加 arXiv 链接，或去「导入」批量粘贴。
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
