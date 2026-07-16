import { useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import {
  useCreateGraph,
  useGroup,
  useGroupGraphs,
  useGroupMembers,
  useImportLibraryToGraph,
} from '../api/hooks';

export function GroupDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data: group, isLoading } = useGroup(id);
  const { data: graphs } = useGroupGraphs(id);
  const { data: members } = useGroupMembers(id);
  const createGraph = useCreateGraph(id ?? '');
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');

  if (isLoading) {
    return <p className="p-6 text-sm text-ink-400">加载…</p>;
  }
  if (!group) {
    return <p className="p-6 text-sm text-rose-600">组不存在或无权访问</p>;
  }

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-6">
      <div>
        <Link to="/groups" className="text-sm text-ink-500 hover:text-ink-800">
          ← 全部组
        </Link>
        <h1 className="text-xl font-semibold text-ink-900 mt-2">{group.name}</h1>
        {group.description && (
          <p className="text-sm text-ink-500 mt-1">{group.description}</p>
        )}
        <p className="text-xs text-ink-400 mt-1">
          你的角色：{group.my_role} · {group.member_count} 成员 · {group.graph_count} 图
        </p>
      </div>

      <section>
        <h2 className="font-medium text-ink-800 mb-2">图（地图工作区）</h2>
        <form
          className="flex flex-wrap gap-2 items-end mb-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (!name.trim() || !id) return;
            createGraph.mutate(
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
          <input
            className="border border-ink-200 rounded px-2 py-1 text-sm w-40"
            placeholder="新图名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <input
            className="border border-ink-200 rounded px-2 py-1 text-sm w-48"
            placeholder="说明（可选）"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
          />
          <button
            type="submit"
            disabled={!name.trim() || createGraph.isPending}
            className="px-3 py-1 rounded bg-ink-800 text-white text-sm disabled:opacity-40"
          >
            新建图
          </button>
        </form>

        <ul className="space-y-2">
          {(graphs ?? []).map((g) => (
            <GraphRow key={g.id} graph={g} groupId={group.id} />
          ))}
          {graphs && graphs.length === 0 && (
            <p className="text-sm text-ink-400">还没有图，先新建一张。</p>
          )}
        </ul>
      </section>

      <section>
        <h2 className="font-medium text-ink-800 mb-2">成员</h2>
        <ul className="text-sm text-ink-700 space-y-1">
          {(members ?? []).map((m) => (
            <li key={m.user_id} className="flex gap-3">
              <span className="font-medium">{m.name}</span>
              <span className="text-ink-400">{m.email}</span>
              <span className="text-ink-400 ml-auto">{m.role}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

function GraphRow({
  graph,
  groupId,
}: {
  graph: { id: string; name: string; description: string; work_count: number };
  groupId: string;
}) {
  const importLib = useImportLibraryToGraph(graph.id);
  return (
    <li className="border border-ink-100 rounded-lg p-3 bg-white flex items-center gap-3">
      <div className="min-w-0 flex-1">
        <div className="font-medium text-ink-900">{graph.name}</div>
        {graph.description && (
          <div className="text-xs text-ink-500">{graph.description}</div>
        )}
        <div className="text-xs text-ink-400 mt-0.5">{graph.work_count} 篇论文</div>
      </div>
      <button
        type="button"
        className="text-xs text-ink-500 hover:underline shrink-0"
        disabled={importLib.isPending}
        onClick={() => {
          if (confirm('将库内全部论文加入此图？')) importLib.mutate();
        }}
      >
        {importLib.isPending ? '导入中…' : '导入全库'}
      </button>
      <Link
        to={`/map?group=${groupId}&graph=${graph.id}`}
        className="px-3 py-1.5 rounded bg-ink-800 text-white text-sm shrink-0"
      >
        打开地图
      </Link>
    </li>
  );
}
