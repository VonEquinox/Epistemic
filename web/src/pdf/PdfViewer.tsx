import {
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  forwardRef,
} from 'react';
import type { PDFDocumentProxy, PDFPageProxy } from 'pdfjs-dist';
import { fetchPdfBlobUrl, loadPdfFromBlob, uploadPdf } from './load';
import { bboxToViewport, parseBBox, type PdfBBox } from './coords';

export interface EvidenceTarget {
  id?: string;
  page: number;
  text: string;
  bbox?: unknown;
}

export interface PdfViewerHandle {
  jumpToEvidence: (ev: EvidenceTarget) => void;
  jumpToPage: (page: number) => void;
}

interface Props {
  versionId: string | null | undefined;
  hasPdf: boolean;
  evidences?: EvidenceTarget[];
  activeEvidenceId?: string | null;
  onSelection?: (sel: {
    text: string;
    page: number;
    bbox?: PdfBBox;
  }) => void;
  onUploaded?: () => void;
  className?: string;
}

const SCALE = 1.15;

export const PdfViewer = forwardRef<PdfViewerHandle, Props>(function PdfViewer(
  {
    versionId,
    hasPdf,
    evidences = [],
    activeEvidenceId,
    onSelection,
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
  const pageEls = useRef<Map<number, HTMLDivElement>>(new Map());

  const cleanup = useCallback(() => {
    if (blobUrlRef.current) {
      URL.revokeObjectURL(blobUrlRef.current);
      blobUrlRef.current = null;
    }
    setDoc(null);
    setNumPages(0);
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
  }, [versionId, hasPdf, cleanup]);

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

  // jump when activeEvidenceId changes
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
      onUploaded?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

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
        <p className="text-ink-500">尚未上传 PDF</p>
        <label className="px-3 py-2 rounded-md bg-ink-900 text-white text-xs cursor-pointer">
          上传 PDF
          <input
            type="file"
            accept="application/pdf"
            className="hidden"
            onChange={(e) => onFile(e.target.files?.[0] ?? null)}
          />
        </label>
        {error && <p className="text-rose-600 text-xs">{error}</p>}
      </div>
    );
  }

  return (
    <div className={`flex flex-col min-h-0 ${className ?? ''}`}>
      <div className="h-9 border-b border-ink-100 px-3 flex items-center gap-3 text-xs text-ink-500 bg-white">
        <span>PDF</span>
        {numPages > 0 && <span>{numPages} 页</span>}
        {loading && <span>加载中…</span>}
        {error && <span className="text-rose-600">{error}</span>}
        <label className="ml-auto text-accent cursor-pointer hover:underline">
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
        className="flex-1 overflow-y-auto bg-ink-100 p-3 space-y-4"
      >
        {doc &&
          Array.from({ length: numPages }, (_, i) => i + 1).map((pageNum) => (
            <PdfPage
              key={pageNum}
              doc={doc}
              pageNumber={pageNum}
              evidences={evidences.filter((e) => e.page === pageNum)}
              flashId={flashId}
              focus={focusPage === pageNum}
              onMount={(el) => {
                if (el) pageEls.current.set(pageNum, el);
                else pageEls.current.delete(pageNum);
              }}
              onSelection={onSelection}
            />
          ))}
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
  onSelection,
}: {
  doc: PDFDocumentProxy;
  pageNumber: number;
  evidences: EvidenceTarget[];
  flashId: string | null;
  focus: boolean;
  onMount: (el: HTMLDivElement | null) => void;
  onSelection?: Props['onSelection'];
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [viewportSize, setViewportSize] = useState({ w: 0, h: 0 });
  const [pageProxy, setPageProxy] = useState<PDFPageProxy | null>(null);
  const [textHighlights, setTextHighlights] = useState<
    { id: string; rect: { left: number; top: number; width: number; height: number }; flash: boolean }[]
  >([]);

  useEffect(() => {
    onMount(wrapRef.current);
    return () => onMount(null);
  }, [onMount]);

  useEffect(() => {
    let cancelled = false;
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
      await (
        page.render as (params: {
          canvasContext: CanvasRenderingContext2D;
          viewport: ReturnType<PDFPageProxy['getViewport']>;
        }) => { promise: Promise<void> }
      )({ canvasContext: ctx, viewport }).promise;
    })();
    return () => {
      cancelled = true;
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
        // text fallback: find first matching item
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
    if (!onSelection || !pageProxy) return;
    const sel = window.getSelection();
    const text = sel?.toString().trim();
    if (!text) return;
    // approximate bbox from selection range if inside this page
    const range = sel?.rangeCount ? sel.getRangeAt(0) : null;
    if (!range || !wrapRef.current?.contains(range.commonAncestorContainer)) return;
    onSelection({ text, page: pageNumber });
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
      <div className="absolute left-2 top-2 z-10 text-[10px] bg-ink-900/70 text-white px-1.5 py-0.5 rounded">
        p.{pageNumber}
      </div>
      <canvas ref={canvasRef} className="block" />
      {/* transparent text layer for selection — simplified: use highlights only */}
      <div className="absolute inset-0 pointer-events-none">
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
