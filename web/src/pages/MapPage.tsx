import { useEffect, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import {
  useCreateSavedView,
  useDeleteSavedView,
  useGraph,
  useGroup,
  useGroups,
  useMap,
  useSavedViews,
} from '../api/hooks';
import { MapView } from '../graph/MapView';
import { ASPECTS } from '../graph/aspects';
import { Drawer } from '../components/Drawer';
import { useUiStore } from '../stores/ui';

export function MapPage() {
  const [params, setParams] = useSearchParams();
  const graphIdFromUrl = params.get('graph');
  const groupIdFromUrl = params.get('group');

  const setActiveGroupId = useUiStore((s) => s.setActiveGroupId);
  const setActiveGraphId = useUiStore((s) => s.setActiveGraphId);
  const activeGraphId = useUiStore((s) => s.activeGraphId);
  const activeGroupId = useUiStore((s) => s.activeGroupId);

  const graphId = graphIdFromUrl ?? activeGraphId;
  const groupId = groupIdFromUrl ?? activeGroupId;

  useEffect(() => {
    if (graphIdFromUrl) setActiveGraphId(graphIdFromUrl);
    if (groupIdFromUrl) setActiveGroupId(groupIdFromUrl);
  }, [graphIdFromUrl, groupIdFromUrl, setActiveGraphId, setActiveGroupId]);

  const { data: groups } = useGroups();
  const { data: groupMeta } = useGroup(groupId ?? undefined);
  const { data: graphMeta } = useGraph(graphId ?? undefined);

  const { data, isLoading, error } = useMap(graphId);
  const selectWork = useUiStore((s) => s.selectWork);
  const weights = useUiStore((s) => s.weights);
  const setWeights = useUiStore((s) => s.setWeights);
  const topicEnabled = useUiStore((s) => s.topicEnabled);
  const setTopicEnabled = useUiStore((s) => s.setTopicEnabled);
  const activeAspect = useUiStore((s) => s.activeAspect);
  const setActiveAspect = useUiStore((s) => s.setActiveAspect);
  const showAssertionEdges = useUiStore((s) => s.showAssertionEdges);
  const setShowAssertionEdges = useUiStore((s) => s.setShowAssertionEdges);
  const minSimScore = useUiStore((s) => s.minSimScore);
  const setMinSimScore = useUiStore((s) => s.setMinSimScore);
  const lod = useUiStore((s) => s.lod);
  const nav = useNavigate();
  const [viewName, setViewName] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const { data: views } = useSavedViews();
  const createView = useCreateSavedView();
  const deleteView = useDeleteSavedView();

  const scopeLabel = graphMeta
    ? `${groupMeta?.name ?? '组'} / ${graphMeta.name}`
    : graphId
      ? '图'
      : '全库地图';

  return (
    <div className="h-full relative flex flex-col">
      <div className="border-b border-ink-100 bg-white px-4 py-2 flex flex-col gap-2 text-xs text-ink-600">
        <div className="flex items-center gap-3 flex-wrap">
          <span className="font-medium text-ink-800">地图</span>
          <span className="text-ink-500 truncate max-w-[14rem]" title={scopeLabel}>
            {scopeLabel}
          </span>
          <span className="text-ink-400">LOD: {lod}</span>
          {data && (
            <span className="text-ink-400">{data.nodes.length} 节点</span>
          )}

          <Link to="/groups" className="text-accent hover:underline">
            切换组/图
          </Link>
          {graphId && (
            <button
              type="button"
              className="text-ink-400 hover:underline"
              onClick={() => {
                setActiveGraphId(null);
                setParams({});
              }}
            >
              看全库
            </button>
          )}

          {!graphId && groups && groups.length > 0 && (
            <span className="text-amber-700">
              当前为全库；可从
              <Link to="/groups" className="underline ml-0.5">
                研究组
              </Link>
              打开某张图
            </span>
          )}

          <div className="flex items-center gap-1 flex-wrap">
            <span className="text-ink-500 shrink-0">分析层</span>
            {ASPECTS.map((a) => {
              const on = activeAspect === a.key;
              return (
                <button
                  key={a.key}
                  type="button"
                  className={
                    on
                      ? 'px-2 py-0.5 rounded bg-ink-800 text-white'
                      : 'px-2 py-0.5 rounded border border-ink-200 hover:bg-ink-50'
                  }
                  onClick={() => setActiveAspect(a.key)}
                >
                  {a.label}
                </button>
              );
            })}
            <button
              type="button"
              className={
                activeAspect === null
                  ? 'px-2 py-0.5 rounded bg-ink-800 text-white'
                  : 'px-2 py-0.5 rounded border border-ink-200 hover:bg-ink-50'
              }
              onClick={() => setActiveAspect(null)}
              title="引用耦合 + 方法谱系综合布局，不画相似边"
            >
              综合布局
            </button>
          </div>

          <label className="flex items-center gap-1 ml-1">
            <input
              type="checkbox"
              checked={showAssertionEdges}
              onChange={(e) => setShowAssertionEdges(e.target.checked)}
            />
            断言边
          </label>

          {activeAspect && (
            <label
              className="flex items-center gap-2 ml-1 border-l border-ink-100 pl-3"
              title="只显示余弦相似度 ≥ 阈值的边。分数越高越相关/越近。"
            >
              <span className="text-ink-500 shrink-0">相关度 ≥</span>
              <input
                type="range"
                min={0.25}
                max={0.9}
                step={0.01}
                value={minSimScore}
                onChange={(e) => setMinSimScore(Number(e.target.value))}
                className="w-28 accent-ink-800"
              />
              <span className="tabular-nums font-medium text-ink-800 w-10">
                {minSimScore.toFixed(2)}
              </span>
            </label>
          )}

          <button
            type="button"
            className="text-ink-400 hover:underline"
            onClick={() => setShowAdvanced((v) => !v)}
          >
            {showAdvanced ? '收起高级' : '高级'}
          </button>

          <span className="ml-auto text-ink-400">
            {activeAspect
              ? '相似边实时按相关度过滤 · 悬停显示分数'
              : '综合布局 · 相似不画边 · 近景可开断言边'}
          </span>
        </div>

        {showAdvanced && (
          <div className="flex items-center gap-4 flex-wrap border-t border-ink-50 pt-2">
            <label className="flex items-center gap-1">
              引用耦合
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={weights.citation_coupling}
                onChange={(e) =>
                  setWeights({ citation_coupling: Number(e.target.value) })
                }
              />
            </label>
            <label className="flex items-center gap-1">
              方法谱系
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={weights.method_lineage}
                onChange={(e) =>
                  setWeights({ method_lineage: Number(e.target.value) })
                }
              />
            </label>
            <label className="flex items-center gap-1">
              <input
                type="checkbox"
                checked={topicEnabled}
                onChange={(e) => setTopicEnabled(e.target.checked)}
              />
              旧主题引力
            </label>

            <div className="flex items-center gap-1 ml-2 border-l border-ink-100 pl-3">
              <span className="text-ink-400">视角</span>
              <select
                className="border border-ink-200 rounded px-1 max-w-[9rem]"
                defaultValue=""
                onChange={(e) => {
                  const v = views?.find((x) => x.id === e.target.value);
                  if (!v) return;
                  setWeights({
                    citation_coupling: v.weights.citation_coupling ?? 0.6,
                    method_lineage: v.weights.method_lineage ?? 0.4,
                    topic: v.weights.topic ?? 0,
                  });
                  if ((v.weights.topic ?? 0) > 0) setTopicEnabled(true);
                }}
              >
                <option value="">选择已存…</option>
                {views?.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name}
                  </option>
                ))}
              </select>
              <input
                className="border border-ink-200 rounded px-1 w-24"
                placeholder="命名"
                value={viewName}
                onChange={(e) => setViewName(e.target.value)}
              />
              <button
                type="button"
                className="px-2 py-0.5 rounded bg-ink-800 text-white disabled:opacity-40"
                disabled={!viewName.trim() || createView.isPending}
                onClick={() =>
                  createView.mutate(
                    { name: viewName.trim(), weights },
                    { onSuccess: () => setViewName('') },
                  )
                }
              >
                保存
              </button>
              {views && views.length > 0 && (
                <button
                  type="button"
                  className="text-rose-500 hover:underline"
                  onClick={() => {
                    const last = views[views.length - 1];
                    if (last && confirm(`删除视角「${last.name}」？`)) {
                      deleteView.mutate(last.id);
                    }
                  }}
                >
                  删末条
                </button>
              )}
            </div>
          </div>
        )}
      </div>
      <div className="flex-1 min-h-0 p-3">
        {isLoading && <p className="text-sm text-ink-500 p-4">加载地图…</p>}
        {error && (
          <p className="text-sm text-rose-600 p-4">{(error as Error).message}</p>
        )}
        {data && (
          <MapView
            data={data}
            showCandidates={showAssertionEdges}
            onSelect={(id) => selectWork(id)}
            onOpenEgo={(id) => nav(`/ego/work/${id}`)}
            onSelectEdge={(rid) => nav(`/review?focus=${rid}`)}
          />
        )}
      </div>
      <Drawer />
    </div>
  );
}
