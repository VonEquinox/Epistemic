import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuickAdd, useWorks } from '../api/hooks';

export function PapersPage() {
  const [q, setQ] = useState('');
  const [input, setInput] = useState('');
  const { data, isLoading } = useWorks({ query: q || undefined });
  const quickAdd = useQuickAdd();

  return (
    <div className="max-w-5xl mx-auto p-6 space-y-4">
      <div className="flex gap-3 items-end">
        <label className="flex-1 text-sm">
          <span className="text-ink-600">搜索</span>
          <input
            className="mt-1 w-full border border-ink-200 rounded-md px-3 py-2"
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
            className="flex-1 border border-ink-200 rounded-md px-3 py-2 text-sm"
            placeholder="快速添加：arXiv URL / DOI"
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
          <button
            type="submit"
            className="px-3 py-2 rounded-md bg-ink-900 text-white text-sm"
          >
            添加
          </button>
        </form>
      </div>

      {quickAdd.isError && (
        <p className="text-sm text-rose-600">{(quickAdd.error as Error).message}</p>
      )}

      <div className="bg-white border border-ink-200 rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-ink-50 text-ink-600 text-left">
            <tr>
              <th className="px-3 py-2 font-medium">标题</th>
              <th className="px-3 py-2 font-medium w-20">年份</th>
              <th className="px-3 py-2 font-medium">作者</th>
              <th className="px-3 py-2 font-medium w-32">arXiv</th>
            </tr>
          </thead>
          <tbody>
            {isLoading && (
              <tr>
                <td colSpan={4} className="px-3 py-6 text-ink-400">
                  加载中…
                </td>
              </tr>
            )}
            {data?.map((item) => (
              <tr
                key={item.work.id}
                className="border-t border-ink-100 hover:bg-ink-50"
              >
                <td className="px-3 py-2">
                  <Link
                    to={`/papers/${item.work.id}`}
                    className="text-ink-900 hover:text-accent"
                  >
                    {item.title}
                  </Link>
                </td>
                <td className="px-3 py-2 text-ink-500">{item.year ?? '—'}</td>
                <td className="px-3 py-2 text-ink-600 truncate max-w-[200px]">
                  {item.authors.slice(0, 3).join(', ')}
                  {item.authors.length > 3 && ' et al.'}
                </td>
                <td className="px-3 py-2 font-mono text-xs text-ink-500">
                  {item.arxiv_id ?? '—'}
                </td>
              </tr>
            ))}
            {data?.length === 0 && (
              <tr>
                <td colSpan={4} className="px-3 py-6 text-ink-400">
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
