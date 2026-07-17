import { useState } from 'react';
import { COLORS } from './styles';

/** Compact always-available legend for the map (MD3 card, bottom-left). */
export function MapLegend({ showSimilarity }: { showSimilarity: boolean }) {
  const [open, setOpen] = useState(true);

  if (!open) {
    return (
      <button
        type="button"
        className="absolute bottom-5 left-5 z-10 md-chip bg-surface-container-lowest shadow-elev1"
        onClick={() => setOpen(true)}
      >
        图例
      </button>
    );
  }

  return (
    <div className="absolute bottom-5 left-5 z-10 md-card px-4 py-3 text-xs text-on-surface-variant select-none">
      <div className="flex items-center justify-between gap-6 mb-2">
        <span className="font-medium text-on-surface">图例</span>
        <button
          type="button"
          className="md-icon-btn h-5 w-5 text-sm leading-none"
          aria-label="收起图例"
          onClick={() => setOpen(false)}
        >
          ×
        </button>
      </div>
      <div className="grid grid-cols-[auto_1fr] gap-x-2.5 gap-y-1.5 items-center">
        <NodeSwatch fill={COLORS.node} border={COLORS.readerBorder} borderW={2.5} />
        <span>已读（边框粗细 = 已读人数）</span>
        <NodeSwatch fill={COLORS.nodeUnread} border="#fff" borderW={1.5} />
        <span>无人读</span>
        <NodeSwatch fill={COLORS.node} border={COLORS.disputeDot} borderW={2.5} />
        <span>有未决争议</span>

        <LineSwatch color={COLORS.edgeCandidate} dashed />
        <span>AI 候选（未审）</span>
        <LineSwatch color={COLORS.edgeConfirmed1} />
        <span>已确认</span>
        <LineSwatch color={COLORS.edgeConfirmed2} width={3} />
        <span>多人确认</span>
        <LineSwatch color={COLORS.edgeDisputed} width={3} />
        <span>争议</span>

        {showSimilarity && (
          <>
            <LineSwatch color={COLORS.simEdge} width={2} opacity={0.8} />
            <span>语义相似（当前分析层，非断言）</span>
          </>
        )}
      </div>
    </div>
  );
}

function NodeSwatch({
  fill,
  border,
  borderW,
}: {
  fill: string;
  border: string;
  borderW: number;
}) {
  return (
    <svg width="18" height="14" className="justify-self-center">
      <circle
        cx="9"
        cy="7"
        r="4.5"
        fill={fill}
        stroke={border}
        strokeWidth={borderW}
      />
    </svg>
  );
}

function LineSwatch({
  color,
  dashed = false,
  width = 2,
  opacity = 1,
}: {
  color: string;
  dashed?: boolean;
  width?: number;
  opacity?: number;
}) {
  return (
    <svg width="18" height="14" className="justify-self-center">
      <line
        x1="1"
        y1="7"
        x2="17"
        y2="7"
        stroke={color}
        strokeWidth={width}
        strokeOpacity={opacity}
        strokeDasharray={dashed ? '4 3' : undefined}
        strokeLinecap="round"
      />
    </svg>
  );
}
