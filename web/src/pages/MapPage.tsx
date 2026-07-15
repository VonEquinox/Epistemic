import { useNavigate } from 'react-router-dom';
import { useMap } from '../api/hooks';
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

  return (
    <div className="h-full relative flex flex-col">
      <div className="h-11 border-b border-ink-100 bg-white px-4 flex items-center gap-4 text-xs text-ink-600">
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
        <span className="ml-auto text-ink-400">
          单击卡片 · 双击进入 Ego
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
            onSelect={(id) => selectWork(id)}
            onOpenEgo={(id) => nav(`/ego/work/${id}`)}
          />
        )}
      </div>
      <Drawer />
    </div>
  );
}
