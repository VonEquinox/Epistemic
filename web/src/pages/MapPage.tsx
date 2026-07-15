import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  useCreateSavedView,
  useDeleteSavedView,
  useMap,
  useSavedViews,
} from '../api/hooks';
import { MapView } from '../graph/MapView';
import { Drawer } from '../components/Drawer';
import { useUiStore } from '../stores/ui';

export function MapPage() {
  const { data, isLoading, error } = useMap();
  const selectWork = useUiStore((s) => s.selectWork);
  const weights = useUiStore((s) => s.weights);
  const setWeights = useUiStore((s) => s.setWeights);
  const topicEnabled = useUiStore((s) => s.topicEnabled);
  const setTopicEnabled = useUiStore((s) => s.setTopicEnabled);
  const lod = useUiStore((s) => s.lod);
  const nav = useNavigate();
  const [showCandidates, setShowCandidates] = useState(true);
  const [viewName, setViewName] = useState('');
  const { data: views } = useSavedViews();
  const createView = useCreateSavedView();
  const deleteView = useDeleteSavedView();

  return (
    <div className="h-full relative flex flex-col">
      <div className="h-11 border-b border-ink-100 bg-white px-4 flex items-center gap-4 text-xs text-ink-600 flex-wrap">
        <span className="font-medium text-ink-800">全局语义地图</span>
        <span className="text-ink-400">LOD: {lod}</span>
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
          主题引力
        </label>
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={showCandidates}
            onChange={(e) => setShowCandidates(e.target.checked)}
          />
          AI 候选边
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
              title="删除当前下拉选中的第一个视角（若仅有一个）"
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

        <span className="ml-auto text-ink-400">
          单击卡片 · 双击进入 Ego · 近景断言边 · 边框=阅读人数
        </span>
      </div>
      <div className="flex-1 min-h-0 p-3">
        {isLoading && <p className="text-sm text-ink-500 p-4">加载地图…</p>}
        {error && (
          <p className="text-sm text-rose-600 p-4">{(error as Error).message}</p>
        )}
        {data && (
          <MapView
            data={data}
            showCandidates={showCandidates}
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
