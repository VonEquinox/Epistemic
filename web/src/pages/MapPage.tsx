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
import { MapLegend } from '../graph/MapLegend';
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
  const forceTuning = useUiStore((s) => s.forceTuning);
  const setForceTuning = useUiStore((s) => s.setForceTuning);
  const resetForceTuning = useUiStore((s) => s.resetForceTuning);
  const lod = useUiStore((s) => s.lod);
  const nav = useNavigate();
  const [viewName, setViewName] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(true);
  const { data: views } = useSavedViews();
  const createView = useCreateSavedView();
  const deleteView = useDeleteSavedView();

  const scopeLabel = graphMeta
    ? `${groupMeta?.name ?? '组'} / ${graphMeta.name}`
    : graphId
      ? '图'
      : '全库地图';

  const lodLabel = lod === 'far' ? '远景' : lod === 'mid' ? '中景' : '近景';

  return (
    <div className="h-full relative flex flex-col">
      <div className="border-b border-outline-variant bg-surface-container-low px-4 py-2.5 flex flex-col gap-2 text-xs text-on-surface-variant">
        <div className="flex items-center gap-2.5 flex-wrap">
          <span className="text-sm font-medium text-on-surface">地图</span>
          <span
            className="md-chip-static max-w-[15rem]"
            title={scopeLabel}
          >
            <span className="truncate">{scopeLabel}</span>
          </span>
          <span className="md-chip-static tabular-nums">
            {lodLabel}
            {data ? ` · ${data.nodes.length} 节点` : ''}
          </span>

          <Link to="/groups" className="md-btn-text md-btn-sm">
            切换组/图
          </Link>
          {graphId && (
            <button
              type="button"
              className="md-btn-text md-btn-sm text-on-surface-variant"
              onClick={() => {
                setActiveGraphId(null);
                setParams({});
              }}
            >
              看全库
            </button>
          )}

          {!graphId && groups && groups.length > 0 && (
            <span className="inline-flex items-center h-7 px-3 rounded-lg bg-tertiary-container text-on-tertiary-container">
              当前为全库；可从
              <Link to="/groups" className="underline mx-0.5 font-medium">
                研究组
              </Link>
              打开某张图
            </span>
          )}

          <button
            type="button"
            className="md-btn-text md-btn-sm ml-auto"
            onClick={() => setShowAdvanced((v) => !v)}
          >
            {showAdvanced ? '收起高级' : '高级'}
          </button>
        </div>

        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-on-surface-variant shrink-0">分析层</span>
          <div className="flex items-center gap-1.5 flex-wrap">
            {ASPECTS.map((a) => {
              const on = activeAspect === a.key;
              return (
                <button
                  key={a.key}
                  type="button"
                  className={`md-chip ${on ? 'md-chip-selected' : ''}`}
                  onClick={() => setActiveAspect(a.key)}
                >
                  {on && <span aria-hidden>✓</span>}
                  {a.label}
                </button>
              );
            })}
            <button
              type="button"
              className={`md-chip ${activeAspect === null ? 'md-chip-selected' : ''}`}
              onClick={() => setActiveAspect(null)}
              title="引用耦合 + 方法谱系综合布局，不画相似边"
            >
              {activeAspect === null && <span aria-hidden>✓</span>}
              综合布局
            </button>
          </div>

          <label className="flex items-center gap-2 ml-2 border-l border-outline-variant pl-3 cursor-pointer">
            <input
              type="checkbox"
              className="md-switch"
              checked={showAssertionEdges}
              onChange={(e) => setShowAssertionEdges(e.target.checked)}
            />
            断言边
          </label>

          {activeAspect && (
            <label
              className="flex items-center gap-2 ml-1 border-l border-outline-variant pl-3"
              title="只显示余弦相似度 ≥ 阈值的边。分数越高越相关/越近。"
            >
              <span className="text-on-surface-variant shrink-0">相关度 ≥</span>
              <input
                type="range"
                min={0.25}
                max={0.9}
                step={0.01}
                value={minSimScore}
                onChange={(e) => setMinSimScore(Number(e.target.value))}
                className="w-28"
              />
              <span className="tabular-nums font-medium text-on-surface w-10">
                {minSimScore.toFixed(2)}
              </span>
            </label>
          )}

          <span className="ml-auto text-outline hidden lg:inline">
            {activeAspect
              ? '相似边实时按相关度过滤 · 悬停显示分数'
              : '综合布局 · 相似不画边 · 近景可开断言边'}
          </span>
        </div>

        {showAdvanced && (
          <div className="flex items-center gap-4 flex-wrap border-t border-outline-variant pt-2.5">
            <div className="basis-full flex items-center gap-4 flex-wrap rounded-xl bg-surface-container px-3 py-2">
              <span className="font-medium text-on-surface">力模拟</span>
              <label
                className="flex items-center gap-2"
                title="边的基础弹簧拉力；越大，相关节点收拢越明显。"
              >
                初始引力
                <input
                  type="range"
                  min={0}
                  max={4}
                  step={0.1}
                  value={forceTuning.attractionStrength}
                  onChange={(e) =>
                    setForceTuning({ attractionStrength: Number(e.target.value) })
                  }
                  className="w-28"
                />
                <span className="w-9 tabular-nums text-on-surface">
                  {forceTuning.attractionStrength.toFixed(1)}
                </span>
              </label>
              <label
                className="flex items-center gap-2"
                title="所有节点的基础排斥强度；越大，图越松散。"
              >
                初始斥力
                <input
                  type="range"
                  min={0}
                  max={2}
                  step={0.05}
                  value={forceTuning.repulsionStrength}
                  onChange={(e) =>
                    setForceTuning({ repulsionStrength: Number(e.target.value) })
                  }
                  className="w-28"
                />
                <span className="w-9 tabular-nums text-on-surface">
                  {forceTuning.repulsionStrength.toFixed(2)}
                </span>
              </label>
              <label
                className="flex items-center gap-2"
                title="相关度降低时，引力下降的速度；越大，只有高相关边保留强拉力。"
              >
                引力衰减
                <input
                  type="range"
                  min={1}
                  max={8}
                  step={0.25}
                  value={forceTuning.attractionDecay}
                  onChange={(e) =>
                    setForceTuning({ attractionDecay: Number(e.target.value) })
                  }
                  className="w-28"
                />
                <span className="w-9 tabular-nums text-on-surface">
                  {forceTuning.attractionDecay.toFixed(2)}
                </span>
              </label>
              <label
                className="flex items-center gap-2"
                title="距离增加时，斥力下降的速度；越大，斥力越集中在近距离。"
              >
                斥力衰减
                <input
                  type="range"
                  min={1}
                  max={4}
                  step={0.1}
                  value={forceTuning.repulsionDecay}
                  onChange={(e) =>
                    setForceTuning({ repulsionDecay: Number(e.target.value) })
                  }
                  className="w-28"
                />
                <span className="w-9 tabular-nums text-on-surface">
                  {forceTuning.repulsionDecay.toFixed(1)}
                </span>
              </label>
              <button
                type="button"
                className="md-btn-text md-btn-sm ml-auto"
                onClick={resetForceTuning}
              >
                恢复默认
              </button>
            </div>

            <label className="flex items-center gap-2">
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
            <label className="flex items-center gap-2">
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
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                className="md-switch"
                checked={topicEnabled}
                onChange={(e) => setTopicEnabled(e.target.checked)}
              />
              旧主题引力
            </label>

            <div className="flex items-center gap-2 ml-2 border-l border-outline-variant pl-3">
              <span className="text-on-surface-variant">视角</span>
              <select
                className="md-field max-w-[9rem]"
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
                className="md-field w-24"
                placeholder="命名"
                value={viewName}
                onChange={(e) => setViewName(e.target.value)}
              />
              <button
                type="button"
                className="md-btn-filled md-btn-sm"
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
                  className="md-btn-text md-btn-sm text-error"
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
      <div className="flex-1 min-h-0 p-3 relative">
        {isLoading && (
          <p className="text-sm text-on-surface-variant p-4">加载地图…</p>
        )}
        {error && (
          <p className="text-sm text-error p-4">{(error as Error).message}</p>
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
        {data && <MapLegend showSimilarity={!!activeAspect} />}
      </div>
      <Drawer />
    </div>
  );
}
