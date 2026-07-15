import { useMemo, useRef, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useAnnotations, usePromoteClaim, useWork } from '../api/hooks';
import { PaperCard } from '../components/PaperCard';
import { PdfViewer, type PdfViewerHandle } from '../pdf';
import type { EvidenceSpan } from '../api/types';
import { useQueryClient } from '@tanstack/react-query';

export function PaperDetailPage() {
  const { id } = useParams();
  const [sp] = useSearchParams();
  const { data, isLoading, error } = useWork(id);
  const { data: anns } = useAnnotations(id);
  const pdfRef = useRef<PdfViewerHandle>(null);
  const [activeEv, setActiveEv] = useState<string | null>(sp.get('evidence'));
  const [sel, setSel] = useState<{ text: string; page: number } | null>(null);
  const promote = usePromoteClaim();
  const qc = useQueryClient();

  const version = data?.primary_version;
  const evidences: EvidenceSpan[] = data?.evidence ?? [];

  const targets = useMemo(
    () =>
      evidences.map((e) => ({
        id: e.id,
        page: e.page,
        text: e.text,
        bbox: e.bbox,
      })),
    [evidences],
  );

  // initial jump from query
  useMemo(() => {
    const page = sp.get('page');
    if (page) {
      const n = Number(page);
      if (n > 0) setTimeout(() => pdfRef.current?.jumpToPage(n), 400);
    }
  }, [sp]);

  if (isLoading) return <p className="p-6 text-ink-500">加载中…</p>;
  if (error) return <p className="p-6 text-rose-600">{(error as Error).message}</p>;
  if (!data) return null;

  return (
    <div className="h-[calc(100vh-3rem)] flex min-h-0">
      <div className="w-[420px] shrink-0 border-r border-ink-200 overflow-y-auto p-5 bg-white">
        <PaperCard
          card={data}
          onJumpEvidence={(ev) => {
            setActiveEv(ev.id);
            pdfRef.current?.jumpToEvidence({
              id: ev.id,
              page: ev.page,
              text: ev.text,
              bbox: ev.bbox,
            });
          }}
        />
        {anns && anns.length > 0 && (
          <section className="mt-6 space-y-3">
            <h2 className="font-medium text-ink-800 text-sm">全部批注</h2>
            {anns.map((a) => (
              <div
                key={a.id}
                className="border border-ink-100 rounded-md p-3 text-sm"
              >
                <div className="text-xs text-ink-400 mb-1">
                  {a.kind} · {a.visibility} ·{' '}
                  {new Date(a.created_at).toLocaleString()}
                </div>
                <p>{a.body}</p>
              </div>
            ))}
          </section>
        )}
        {sel && version && (
          <div className="mt-4 border border-accent/30 bg-accent-soft/40 rounded-md p-3 text-sm space-y-2">
            <p className="text-xs text-ink-500">
              划选 p.{sel.page}：{sel.text.slice(0, 120)}
            </p>
            <button
              className="px-2 py-1 rounded bg-ink-900 text-white text-xs"
              disabled={promote.isPending}
              onClick={() => {
                promote.mutate(
                  {
                    work_id: data.work.id,
                    version_id: version.id,
                    claim_text: sel.text,
                    source_text: sel.text,
                    page: sel.page,
                  },
                  {
                    onSuccess: () => {
                      setSel(null);
                      qc.invalidateQueries({ queryKey: ['work', id] });
                    },
                  },
                );
              }}
            >
              升格为 Claim
            </button>
            <button
              className="ml-2 px-2 py-1 rounded border border-ink-200 text-xs"
              onClick={() => setSel(null)}
            >
              取消
            </button>
          </div>
        )}
      </div>
      <div className="flex-1 min-w-0 min-h-0">
        <PdfViewer
          ref={pdfRef}
          versionId={version?.id}
          hasPdf={!!version?.pdf_path}
          evidences={targets}
          activeEvidenceId={activeEv}
          onSelection={(s) => setSel(s)}
          onUploaded={() => {
            qc.invalidateQueries({ queryKey: ['work', id] });
          }}
          className="h-full"
        />
      </div>
    </div>
  );
}
