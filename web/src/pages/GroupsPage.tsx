import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useCreateGroup, useGroups } from '../api/hooks';

export function GroupsPage() {
  const { data, isLoading, error } = useGroups();
  const create = useCreateGroup();
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');

  return (
    <div className="max-w-3xl mx-auto p-4 md:p-6 space-y-4">
      <div>
        <h1 className="text-xl font-medium text-on-surface">研究组</h1>
        <p className="text-sm text-on-surface-variant mt-1">
          选择一个组，查看组内多张图（地图工作区）。每张图是一组论文上的协作视图。
        </p>
      </div>

      <form
        className="flex flex-wrap gap-2 items-end md-card-outlined p-3"
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
        <label className="text-xs text-on-surface-variant">
          新建组
          <input
            className="md-field block mt-0.5 w-48"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="组名"
          />
        </label>
        <label className="text-xs text-on-surface-variant">
          说明
          <input
            className="md-field block mt-0.5 w-64"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            placeholder="可选"
          />
        </label>
        <button
          type="submit"
          disabled={!name.trim() || create.isPending}
          className="md-btn-filled"
        >
          创建
        </button>
      </form>

      {isLoading && <p className="text-sm text-on-surface-variant">加载…</p>}
      {error && (
        <p className="text-sm text-error">{(error as Error).message}</p>
      )}

      <ul className="space-y-2">
        {(data ?? []).map((g) => (
          <li key={g.id}>
            <Link
              to={`/groups/${g.id}`}
              className="block md-card-outlined p-4 hover:shadow-elev1 transition-shadow"
            >
              <div className="flex items-center justify-between gap-3">
                <div>
                  <div className="font-medium text-on-surface">{g.name}</div>
                  {g.description && (
                    <div className="text-sm text-on-surface-variant mt-0.5">{g.description}</div>
                  )}
                </div>
                <div className="flex flex-col items-end gap-1 shrink-0">
                  <span className="md-chip-static">{g.graph_count} 张图</span>
                  <span className="md-chip-static">{g.member_count} 人 · {g.my_role}</span>
                </div>
              </div>
            </Link>
          </li>
        ))}
        {data && data.length === 0 && (
          <p className="text-sm text-on-surface-variant">你还不在任何组里，先创建一个。</p>
        )}
      </ul>
    </div>
  );
}
