import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  forwardRef,
} from 'react';
import { TextLayer } from 'pdfjs-dist';
import type { PDFDocumentProxy, PDFPageProxy } from 'pdfjs-dist';
import { fetchPdfBlobUrl, loadPdfFromBlob, uploadPdf } from './load';
import {
  bboxToViewport,
  parseBBox,
  viewportRectToBBox,
  type PdfBBox,
} from './coords';

export interface EvidenceTarget {
  id?: string;
  page: number;
  text: string;
  bbox?: unknown;
}

export interface PdfSelection {
  text: string;
  page: number;
  bbox?: PdfBBox;
}

export type AnnotationKindOpt = 'note' | 'conjecture' | 'question';
export type VisibilityOpt = 'private' | 'team';

export interface PdfViewerHandle {
  jumpToEvidence: (ev: EvidenceTarget) => void;
  jumpToPage: (page: number) => void;
}

interface Props {
  versionId: string | null | undefined;
  hasPdf: boolean;
  evidences?: EvidenceTarget[];
  activeEvidenceId?: string | null;
  /** Fired when user confirms selection (bubble still shown by parent or viewer). */
  onSelection?: (sel: PdfSelection) => void;
  /** Create annotation from selection bubble. */
  onAddAnnotation?: (payload: {
    text: string;
    page: number;
    bbox?: PdfBBox;
    kind: AnnotationKindOpt;
    visibility: VisibilityOpt;
    body: string;
  }) => void | Promise<void>;
  /** Promote selection to claim. */
  onPromoteClaim?: (sel: PdfSelection) => void | Promise<void>;
  annotationPending?: boolean;
  promotePending?: boolean;
  onUploaded?: () => void;
  className?: string;
}

const SCALE = 1.15;

const KIND_OPTS: { value: AnnotationKindOpt; label: string }[] = [
  { value: 'note', label: '笔记' },
  { value: 'conjecture', label: '猜想' },
  { value: 'question', label: '问题' },
];

const VIS_OPTS: { value: VisibilityOpt; label: string }[] = [
  { value: 'private', label: '私人' },
  { value: 'team', label: '团队' },
];

export const PdfViewer = forwardRef<PdfViewerHandle, Props>(function PdfViewer(
  {
    versionId,
    hasPdf,
    evidences = [],
    activeEvidenceId,
    onSelection,
    onAddAnnotation,
    onPromoteClaim,
    annotationPending,
    promotePending,
    onUploaded,
    className,
  },
  ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const [numPages, setNumPages] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [flashId, setFlashId] = useState<string | null>(null);
  const [focusPage, setFocusPage] = useState<number | null>(null);
  const blobUrlRef = useRef<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const pageEls = useRef<Map<number, HTMLDivElement>>(new Map());

  // Floating selection bubble state (coords relative to scroll container)
  const [bubble, setBubble] = useState<{
    sel: PdfSelection;
    left: number;
    top: number;
  } | null>(null);
  const [kind, setKind] = useState<AnnotationKindOpt>('note');
  const [visibility, setVisibility] = useState<VisibilityOpt>('team');
  const [noteBody, setNoteBody] = useState('');

  const cleanup = useCallback(() => {
    if (blobUrlRef.current) {
      URL.revokeObjectURL(blobUrlRef.current);
      blobUrlRef.current = null;
    }
    setDoc(null);
    setNumPages(0);
    setBubble(null);
  }, []);

  useEffect(() => {
    if (!versionId || !hasPdf) {
      cleanup();
      return;
    }
    let cancelled = false;
    (async () => {
      setLoading(true);
      setError(null);
      try {
        const url = await fetchPdfBlobUrl(versionId);
        if (cancelled) {
          URL.revokeObjectURL(url);
          return;
        }
        blobUrlRef.current = url;
        const res = await fetch(url);
        const blob = await res.blob();
        const pdf = await loadPdfFromBlob(blob);
        if (cancelled) return;
        setDoc(pdf);
        setNumPages(pdf.numPages);
      } catch (e) {
        if (!cancelled) setError((e as Error).message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
      cleanup();
    };
  }, [versionId, hasPdf, reloadKey, cleanup]);

  const jumpToPage = useCallback((page: number) => {
    setFocusPage(page);
    const el = pageEls.current.get(page);
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }, []);

  const jumpToEvidence = useCallback(
    (ev: EvidenceTarget) => {
      jumpToPage(ev.page);
      const id = ev.id ?? `${ev.page}:${ev.text.slice(0, 24)}`;
      setFlashId(id);
      window.setTimeout(() => setFlashId((cur) => (cur === id ? null : cur)), 1800);
    },
    [jumpToPage],
  );

  useImperativeHandle(ref, () => ({ jumpToEvidence, jumpToPage }), [
    jumpToEvidence,
    jumpToPage,
  ]);

  useEffect(() => {
    if (!activeEvidenceId) return;
    const ev = evidences.find((e) => e.id === activeEvidenceId);
    if (ev) jumpToEvidence(ev);
  }, [activeEvidenceId, evidences, jumpToEvidence]);

  const onFile = async (file: File | null) => {
    if (!file || !versionId) return;
    setLoading(true);
    setError(null);
    try {
      await uploadPdf(versionId, file);
      setReloadKey((key) => key + 1);
      onUploaded?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const dismissBubble = useCallback(() => {
    setBubble(null);
    setNoteBody('');
    window.getSelection()?.removeAllRanges();
  }, []);

  const handlePageSelection = useCallback(
    (sel: PdfSelection, clientRect: DOMRect) => {
      const container = containerRef.current;
      if (!container) return;
      const cRect = container.getBoundingClientRect();
      // Position bubble below selection, clamped to container
      const left = Math.max(
        8,
        Math.min(
          clientRect.left - cRect.left + container.scrollLeft,
          container.scrollWidth - 280,
        ),
      );
      const top =
        clientRect.bottom - cRect.top + container.scrollTop + 8;
      setBubble({ sel, left, top });
      setKind('note');
      setVisibility('team');
      setNoteBody(sel.text);
      onSelection?.(sel);
    },
    [onSelection],
  );

  // Dismiss bubble on scroll / outside click
  useEffect(() => {
    if (!bubble) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') dismissBubble();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [bubble, dismissBubble]);

  if (!versionId) {
    return (
      <div className={`flex items-center justify-center text-sm text-ink-400 ${className ?? ''}`}>
        无版本信息
      </div>
    );
  }

  if (!hasPdf) {
    return (
      <div
        className={`flex flex-col items-center justify-center gap-3 p-6 text-sm ${className ?? ''}`}
      >
        <p className="text-on-surface-variant">尚未上传 PDF</p>
        <label className="md-btn-filled md-btn-sm cursor-pointer">
          上传 PDF
          <input
            type="file"
            accept="application/pdf"
            className="hidden"
            onChange={(e) => onFile(e.target.files?.[0] ?? null)}
          />
        </label>
        {error && <p className="text-error text-xs">{error}</p>}
      </div>
    );
  }

  return (
    <div className={`flex flex-col min-h-0 ${className ?? ''}`}>
      <div className="h-9 border-b border-outline-variant px-3 flex items-center gap-3 text-xs text-on-surface-variant bg-surface-container-low">
        <span>PDF</span>
        {numPages > 0 && <span>{numPages} 页</span>}
        {loading && <span>加载中…</span>}
        {error && <span className="text-error">{error}</span>}
        <label className="ml-auto text-primary cursor-pointer hover:underline">
          替换
          <input
            type="file"
            accept="application/pdf"
            className="hidden"
            onChange={(e) => onFile(e.target.files?.[0] ?? null)}
          />
        </label>
      </div>
      <div
        ref={containerRef}
        className="relative flex-1 overflow-y-auto bg-surface-container p-3 space-y-4"
        onScroll={() => {
          /* keep bubble fixed to content via scroll offsets already baked in */
        }}
      >
        {doc &&
          Array.from({ length: numPages }, (_, i) => i + 1).map((pageNum) => (
            <PdfPage
              key={`${reloadKey}:${pageNum}`}
              doc={doc}
              pageNumber={pageNum}
              evidences={evidences.filter((e) => e.page === pageNum)}
              flashId={flashId}
              focus={focusPage === pageNum}
              onMount={(el) => {
                if (el) pageEls.current.set(pageNum, el);
                else pageEls.current.delete(pageNum);
              }}
              onPageSelection={handlePageSelection}
            />
          ))}

        {bubble && (
          <div
            className="absolute z-30 w-[268px] md-card shadow-elev3 p-3 text-xs space-y-2"
            style={{ left: bubble.left, top: bubble.top }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <p className="text-ink-500 leading-snug">
              划选 p.{bubble.sel.page}：
              <span className="text-ink-700">
                {bubble.sel.text.slice(0, 80)}
                {bubble.sel.text.length > 80 ? '…' : ''}
              </span>
            </p>

            <div className="flex items-center gap-1.5 flex-wrap">
              <span className="text-on-surface-variant">类型</span>
              {KIND_OPTS.map((k) => (
                <button
                  key={k.value}
                  type="button"
                  className={`md-chip ${kind === k.value ? 'md-chip-selected' : ''}`}
                  onClick={() => setKind(k.value)}
                >
                  {k.label}
                </button>
              ))}
            </div>

            <div className="flex items-center gap-1.5 flex-wrap">
              <span className="text-on-surface-variant">可见</span>
              {VIS_OPTS.map((v) => (
                <button
                  key={v.value}
                  type="button"
                  className={`md-chip ${visibility === v.value ? 'md-chip-selected' : ''}`}
                  onClick={() => setVisibility(v.value)}
                >
                  {v.label}
                </button>
              ))}
            </div>

            <textarea
              className="md-field w-full text-xs resize-none"
              rows={2}
              value={noteBody}
              onChange={(e) => setNoteBody(e.target.value)}
              placeholder="批注内容…"
            />

            <div className="flex flex-wrap gap-1.5 pt-0.5">
              {onAddAnnotation && (
                <button
                  type="button"
                  disabled={annotationPending || !noteBody.trim()}
                  className="md-btn-filled md-btn-sm"
                  onClick={async () => {
                    await onAddAnnotation({
                      text: bubble.sel.text,
                      page: bubble.sel.page,
                      bbox: bubble.sel.bbox,
                      kind,
                      visibility,
                      body: noteBody.trim(),
                    });
                    dismissBubble();
                  }}
                >
                  {annotationPending ? '提交中…' : '添加批注'}
                </button>
              )}
              {onPromoteClaim && (
                <button
                  type="button"
                  disabled={promotePending}
                  className="md-btn-outlined md-btn-sm"
                  onClick={async () => {
                    await onPromoteClaim(bubble.sel);
                    dismissBubble();
                  }}
                >
                  {promotePending ? '升格中…' : '升格为 Claim'}
                </button>
              )}
              <button
                type="button"
                className="px-2 py-1 rounded border border-ink-200 text-ink-500 ml-auto"
                onClick={dismissBubble}
              >
                取消
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
});

function PdfPage({
  doc,
  pageNumber,
  evidences,
  flashId,
  focus,
  onMount,
  onPageSelection,
}: {
  doc: PDFDocumentProxy;
  pageNumber: number;
  evidences: EvidenceTarget[];
  flashId: string | null;
  focus: boolean;
  onMount: (el: HTMLDivElement | null) => void;
  onPageSelection?: (sel: PdfSelection, clientRect: DOMRect) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textLayerRef = useRef<HTMLDivElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [viewportSize, setViewportSize] = useState({ w: 0, h: 0 });
  const [pageProxy, setPageProxy] = useState<PDFPageProxy | null>(null);
  const [textHighlights, setTextHighlights] = useState<
    { id: string; rect: { left: number; top: number; width: number; height: number }; flash: boolean }[]
  >([]);
  const textLayerInst = useRef<TextLayer | null>(null);

  useEffect(() => {
    onMount(wrapRef.current);
    return () => onMount(null);
  }, [onMount]);

  useEffect(() => {
    let cancelled = false;
    let renderTask: { promise: Promise<void>; cancel: () => void } | null = null;
    (async () => {
      const page = await doc.getPage(pageNumber);
      if (cancelled) return;
      setPageProxy(page);
      const viewport = page.getViewport({ scale: SCALE });
      setViewportSize({ w: viewport.width, h: viewport.height });
      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      // pdfjs-dist v4 types omit canvas; runtime accepts canvasContext+viewport
      renderTask = (
        page.render as (params: {
          canvasContext: CanvasRenderingContext2D;
          viewport: ReturnType<PDFPageProxy['getViewport']>;
        }) => { promise: Promise<void>; cancel: () => void }
      )({ canvasContext: ctx, viewport });
      try {
        await renderTask.promise;
      } catch (error) {
        if (cancelled) return;
        throw error;
      }

      // Text layer for selectable text
      const textLayerDiv = textLayerRef.current;
      if (!textLayerDiv || cancelled) return;
      textLayerInst.current?.cancel();
      textLayerDiv.innerHTML = '';
      textLayerDiv.style.width = `${viewport.width}px`;
      textLayerDiv.style.height = `${viewport.height}px`;
      try {
        const textContent = await page.getTextContent();
        if (cancelled) return;
        const layer = new TextLayer({
          textContentSource: textContent,
          container: textLayerDiv,
          viewport,
        });
        textLayerInst.current = layer;
        await layer.render();
      } catch {
        // Text layer optional — selection may be limited without it
      }
    })();
    return () => {
      cancelled = true;
      renderTask?.cancel();
      textLayerInst.current?.cancel();
      textLayerInst.current = null;
    };
  }, [doc, pageNumber]);

  // Build highlight rects: prefer bbox; fallback text search on page textContent
  useEffect(() => {
    if (!pageProxy || viewportSize.w === 0) return;
    let cancelled = false;
    (async () => {
      const viewport = pageProxy.getViewport({ scale: SCALE });
      const rects: {
        id: string;
        rect: { left: number; top: number; width: number; height: number };
        flash: boolean;
      }[] = [];

      for (const ev of evidences) {
        const id = ev.id ?? `${ev.page}:${ev.text.slice(0, 24)}`;
        const bbox = parseBBox(ev.bbox);
        if (bbox) {
          rects.push({
            id,
            rect: bboxToViewport(bbox, viewport),
            flash: flashId === id,
          });
          continue;
        }
        try {
          const tc = await pageProxy.getTextContent();
          const needle = ev.text.slice(0, 40).toLowerCase();
          for (const item of tc.items) {
            if (!('str' in item)) continue;
            const str = (item as { str: string }).str;
            if (!str || !needle || !str.toLowerCase().includes(needle.slice(0, 12))) {
              continue;
            }
            const tr = pageProxy.getViewport({ scale: SCALE }).convertToViewportRectangle([
              (item as { transform: number[] }).transform[4],
              (item as { transform: number[] }).transform[5],
              (item as { transform: number[] }).transform[4] +
                ((item as { width?: number }).width ?? 80),
              (item as { transform: number[] }).transform[5] + 12,
            ]);
            const left = Math.min(tr[0], tr[2]);
            const top = Math.min(tr[1], tr[3]);
            rects.push({
              id,
              rect: {
                left,
                top,
                width: Math.abs(tr[2] - tr[0]),
                height: Math.max(14, Math.abs(tr[3] - tr[1])),
              },
              flash: flashId === id,
            });
            break;
          }
        } catch {
          /* ignore */
        }
      }
      if (!cancelled) setTextHighlights(rects);
    })();
    return () => {
      cancelled = true;
    };
  }, [pageProxy, evidences, flashId, viewportSize]);

  const handleMouseUp = () => {
    if (!onPageSelection || !pageProxy || !wrapRef.current) return;
    const sel = window.getSelection();
    const text = sel?.toString().replace(/\s+/g, ' ').trim();
    if (!text) return;
    const range = sel?.rangeCount ? sel.getRangeAt(0) : null;
    if (!range || !wrapRef.current.contains(range.commonAncestorContainer)) return;

    const clientRect = range.getBoundingClientRect();
    if (clientRect.width === 0 && clientRect.height === 0) return;

    const pageRect = wrapRef.current.getBoundingClientRect();
    const cssRect = {
      left: clientRect.left - pageRect.left,
      top: clientRect.top - pageRect.top,
      width: clientRect.width,
      height: clientRect.height,
    };
    const viewport = pageProxy.getViewport({ scale: SCALE });
    let bbox: PdfBBox | undefined;
    try {
      bbox = viewportRectToBBox(cssRect, viewport);
    } catch {
      bbox = undefined;
    }

    onPageSelection({ text, page: pageNumber, bbox }, clientRect);
  };

  return (
    <div
      ref={wrapRef}
      className={`relative mx-auto bg-white shadow-sm ${
        focus ? 'ring-2 ring-accent' : ''
      }`}
      style={{ width: viewportSize.w || '100%' }}
      onMouseUp={handleMouseUp}
      data-page={pageNumber}
    >
      <div className="absolute left-2 top-2 z-20 text-[10px] bg-[rgba(46,48,54,0.82)] text-[#f0f0f7] px-1.5 py-0.5 rounded-md pointer-events-none">
        p.{pageNumber}
      </div>
      <canvas ref={canvasRef} className="block" />
      {/* PDF.js text layer — enables native text selection */}
      <div ref={textLayerRef} className="textLayer" />
      {/* Evidence / claim highlight overlays (non-interactive) */}
      <div className="absolute inset-0 pointer-events-none z-[1]">
        {textHighlights.map((h) => (
          <div
            key={h.id}
            className={`absolute rounded-sm border ${
              h.flash
                ? 'bg-amber-300/50 border-amber-500 animate-pulse'
                : 'bg-accent/20 border-accent/40'
            }`}
            style={{
              left: h.rect.left,
              top: h.rect.top,
              width: h.rect.width,
              height: h.rect.height,
            }}
          />
        ))}
      </div>
    </div>
  );
}
