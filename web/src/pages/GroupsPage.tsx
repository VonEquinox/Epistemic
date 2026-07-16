import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useCreateGroup, useGroups } from '../api/hooks';

export function GroupsPage() {
  const { data, isLoading, error } = useGroups();
  const create = useCreateGroup();
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-6">
      <div>
        <h1 className="text-xl font-semibold text-ink-900">研究组</h1>
        <p className="text-sm text-ink-500 mt-1">
          选择一个组，查看组内多张图（地图工作区）。每张图是一组论文上的协作视图。
        </p>
      </div>

      <form
        className="flex flex-wrap gap-2 items-end border border-ink-100 rounded-lg p-3 bg-white"
        onSubmit={(e) => {
          e.preventDefault();
          if (!name.trim()) return;
          create.mutate(
            { name: name.trim(), description: desc.trim() || undefined },
            {
              onSuccess: () => {
                setName('');
                setDesc('');
              },
            },
          );
        }}
      >
        <label className="text-xs text-ink-500">
          新建组
          <input
            className="block mt-0.5 border border-ink-200 rounded px-2 py-1 text-sm w-48"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="组名"
          />
        </label>
        <label className="text-xs text-ink-500">
          说明
          <input
            className="block mt-0.5 border border-ink-200 rounded px-2 py-1 text-sm w-64"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            placeholder="可选"
          />
        </label>
        <button
          type="submit"
          disabled={!name.trim() || create.isPending}
          className="px-3 py-1.5 rounded bg-ink-800 text-white text-sm disabled:opacity-40"
        >
          创建
        </button>
      </form>

      {isLoading && <p className="text-sm text-ink-400">加载…</p>}
      {error && (
        <p className="text-sm text-rose-600">{(error as Error).message}</p>
      )}

      <ul className="space-y-2">
        {(data ?? []).map((g) => (
          <li key={g.id}>
            <Link
              to={`/groups/${g.id}`}
              className="block border border-ink-100 rounded-lg p-4 bg-white hover:border-ink-300"
            >
              <div className="flex items-center justify-between gap-3">
                <div>
                  <div className="font-medium text-ink-900">{g.name}</div>
                  {g.description && (
                    <div className="text-sm text-ink-500 mt-0.5">{g.description}</div>
                  )}
                </div>
                <div className="text-xs text-ink-400 shrink-0 text-right">
                  <div>{g.graph_count} 张图</div>
                  <div>{g.member_count} 人 · {g.my_role}</div>
                </div>
              </div>
            </Link>
          </li>
        ))}
        {data && data.length === 0 && (
          <p className="text-sm text-ink-400">你还不在任何组里，先创建一个。</p>
        )}
      </ul>
    </div>
  );
}
