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
    return <p className="p-6 text-sm text-on-surface-variant">加载…</p>;
  }
  if (!group) {
    return <p className="p-6 text-sm text-error">组不存在或无权访问</p>;
  }

  return (
    <div className="max-w-3xl mx-auto p-4 md:p-6 space-y-6">
      <div>
        <Link to="/groups" className="md-btn-text md-btn-sm -ml-3">
          ← 全部组
        </Link>
        <h1 className="text-xl font-medium text-on-surface mt-2">{group.name}</h1>
        {group.description && (
          <p className="text-sm text-on-surface-variant mt-1">{group.description}</p>
        )}
        <div className="flex flex-wrap items-center gap-1.5 mt-2">
          <span className="md-chip-static">你的角色：{group.my_role}</span>
          <span className="md-chip-static">{group.member_count} 成员</span>
          <span className="md-chip-static">{group.graph_count} 图</span>
        </div>
      </div>

      <section>
        <h2 className="text-sm font-medium text-on-surface mb-2">图（地图工作区）</h2>
        <form
          className="flex flex-wrap gap-2 items-center mb-3"
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
            className="md-field w-40"
            placeholder="新图名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <input
            className="md-field w-48"
            placeholder="说明（可选）"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
          />
          <button
            type="submit"
            disabled={!name.trim() || createGraph.isPending}
            className="md-btn-filled"
          >
            新建图
          </button>
        </form>

        <ul className="space-y-2">
          {(graphs ?? []).map((g) => (
            <GraphRow key={g.id} graph={g} groupId={group.id} />
          ))}
          {graphs && graphs.length === 0 && (
            <p className="text-sm text-on-surface-variant">还没有图，先新建一张。</p>
          )}
        </ul>
      </section>

      <section>
        <h2 className="text-sm font-medium text-on-surface mb-2">成员</h2>
        <ul className="md-card-outlined divide-y divide-outline-variant text-sm">
          {(members ?? []).map((m) => (
            <li key={m.user_id} className="flex items-center gap-3 px-4 py-2.5">
              <span className="font-medium text-on-surface">{m.name}</span>
              <span className="text-on-surface-variant">{m.email}</span>
              <span className="md-chip-static ml-auto">{m.role}</span>
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
    <li className="md-card-outlined p-3 flex items-center gap-3 hover:shadow-elev1 transition-shadow">
      <div className="min-w-0 flex-1">
        <div className="font-medium text-on-surface">{graph.name}</div>
        {graph.description && (
          <div className="text-xs text-on-surface-variant">{graph.description}</div>
        )}
        <span className="md-chip-static mt-1.5">{graph.work_count} 篇论文</span>
      </div>
      <button
        type="button"
        className="md-btn-text md-btn-sm shrink-0"
        disabled={importLib.isPending}
        onClick={() => {
          if (confirm('将库内全部论文加入此图？')) importLib.mutate();
        }}
      >
        {importLib.isPending ? '导入中…' : '导入全库'}
      </button>
      <Link
        to={`/map?group=${groupId}&graph=${graph.id}`}
        className="md-btn-tonal shrink-0"
      >
        打开地图
      </Link>
    </li>
  );
}
