import { useMemo, useRef, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import {
  useAnnotations,
  useCreateAnnotation,
  usePromoteClaim,
  useWork,
} from '../api/hooks';
import { PaperCard } from '../components/PaperCard';
import { PdfViewer, type PdfViewerHandle } from '../pdf';
import type { Annotation, EvidenceSpan } from '../api/types';
import { useQueryClient } from '@tanstack/react-query';

const KIND_LABEL: Record<string, string> = {
  note: '笔记',
  conjecture: '猜想',
  question: '问题',
};

const VIS_LABEL: Record<string, string> = {
  private: '私人',
  team: '团队',
};

export function PaperDetailPage() {
  const { id } = useParams();
  const [sp] = useSearchParams();
  const { data, isLoading, error } = useWork(id);
  const { data: anns } = useAnnotations(id);
  const pdfRef = useRef<PdfViewerHandle>(null);
  const [activeEv, setActiveEv] = useState<string | null>(sp.get('evidence'));
  const promote = usePromoteClaim();
  const createAnn = useCreateAnnotation(id ?? '');
  const qc = useQueryClient();
  const [replyingTo, setReplyingTo] = useState<string | null>(null);
  const [replyBody, setReplyBody] = useState('');

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

  // Group annotations: top-level + replies
  const { roots, childrenOf } = useMemo(() => {
    const list = anns ?? [];
    const childrenOf = new Map<string, Annotation[]>();
    const roots: Annotation[] = [];
    for (const a of list) {
      if (a.parent_id) {
        const arr = childrenOf.get(a.parent_id) ?? [];
        arr.push(a);
        childrenOf.set(a.parent_id, arr);
      } else {
        roots.push(a);
      }
    }
    return { roots, childrenOf };
  }, [anns]);

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

  const submitReply = (parentId: string) => {
    if (!replyBody.trim() || !id) return;
    createAnn.mutate(
      {
        body: replyBody.trim(),
        kind: 'note',
        visibility: 'team',
        parent_id: parentId,
        version_id: version?.id,
      },
      {
        onSuccess: () => {
          setReplyingTo(null);
          setReplyBody('');
        },
      },
    );
  };

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

        <section className="mt-6 space-y-3">
          <h2 className="font-medium text-ink-800 text-sm">
            全部批注{anns ? ` (${anns.length})` : ''}
          </h2>
          {roots.length === 0 && (
            <p className="text-xs text-ink-400">
              暂无批注。在 PDF 中划选文字可添加。
            </p>
          )}
          {roots.map((a) => (
            <AnnotationItem
              key={a.id}
              ann={a}
              replies={childrenOf.get(a.id) ?? []}
              replying={replyingTo === a.id}
              replyBody={replyBody}
              replyPending={createAnn.isPending}
              onJump={() => {
                const anchor = a.anchor as
                  | { page?: number; text?: string; bbox?: unknown }
                  | null
                  | undefined;
                if (anchor?.page) {
                  pdfRef.current?.jumpToEvidence({
                    page: anchor.page,
                    text: anchor.text ?? a.body,
                    bbox: anchor.bbox,
                  });
                }
              }}
              onStartReply={() => {
                setReplyingTo(a.id);
                setReplyBody('');
              }}
              onCancelReply={() => {
                setReplyingTo(null);
                setReplyBody('');
              }}
              onChangeReply={setReplyBody}
              onSubmitReply={() => submitReply(a.id)}
            />
          ))}
        </section>
      </div>
      <div className="flex-1 min-w-0 min-h-0">
        <PdfViewer
          ref={pdfRef}
          versionId={version?.id}
          hasPdf={!!version?.pdf_path}
          evidences={targets}
          activeEvidenceId={activeEv}
          annotationPending={createAnn.isPending}
          promotePending={promote.isPending}
          onAddAnnotation={async ({ text, page, bbox, kind, visibility, body }) => {
            if (!id) return;
            await createAnn.mutateAsync({
              body,
              kind,
              visibility,
              version_id: version?.id,
              anchor: {
                page,
                text,
                ...(bbox ? { bbox } : {}),
              },
            });
          }}
          onPromoteClaim={async (sel) => {
            if (!version) return;
            await promote.mutateAsync({
              work_id: data.work.id,
              version_id: version.id,
              claim_text: sel.text,
              source_text: sel.text,
              page: sel.page,
              bbox: sel.bbox,
            });
            qc.invalidateQueries({ queryKey: ['work', id] });
          }}
          onUploaded={() => {
            qc.invalidateQueries({ queryKey: ['work', id] });
          }}
          className="h-full"
        />
      </div>
    </div>
  );
}

function AnnotationItem({
  ann,
  replies,
  replying,
  replyBody,
  replyPending,
  onJump,
  onStartReply,
  onCancelReply,
  onChangeReply,
  onSubmitReply,
}: {
  ann: Annotation;
  replies: Annotation[];
  replying: boolean;
  replyBody: string;
  replyPending: boolean;
  onJump: () => void;
  onStartReply: () => void;
  onCancelReply: () => void;
  onChangeReply: (v: string) => void;
  onSubmitReply: () => void;
}) {
  const anchor = ann.anchor as { page?: number; text?: string } | null | undefined;
  return (
    <div className="border border-ink-100 rounded-md p-3 text-sm space-y-2">
      <div className="flex items-center gap-1.5 text-xs text-ink-400 flex-wrap">
        <span className="px-1.5 py-0.5 rounded bg-ink-100 text-ink-700">
          {KIND_LABEL[ann.kind] ?? ann.kind}
        </span>
        <span className="px-1.5 py-0.5 rounded bg-ink-50 text-ink-500">
          {VIS_LABEL[ann.visibility] ?? ann.visibility}
        </span>
        <span className="ml-auto">{new Date(ann.created_at).toLocaleString()}</span>
      </div>
      <p className="text-ink-800 whitespace-pre-wrap">{ann.body}</p>
      {anchor?.page != null && (
        <button
          type="button"
          className="text-xs text-accent hover:underline"
          onClick={onJump}
        >
          定位 p.{anchor.page}
          {anchor.text ? `：${anchor.text.slice(0, 40)}${anchor.text.length > 40 ? '…' : ''}` : ''}
        </button>
      )}
      {replies.length > 0 && (
        <div className="ml-3 border-l-2 border-ink-100 pl-3 space-y-2">
          {replies.map((r) => (
            <div key={r.id} className="text-xs">
              <div className="text-ink-400 mb-0.5">
                {KIND_LABEL[r.kind] ?? r.kind} ·{' '}
                {new Date(r.created_at).toLocaleString()}
              </div>
              <p className="text-ink-700 whitespace-pre-wrap">{r.body}</p>
            </div>
          ))}
        </div>
      )}
      {!replying ? (
        <button
          type="button"
          className="text-xs text-ink-500 hover:text-accent"
          onClick={onStartReply}
        >
          回复
        </button>
      ) : (
        <div className="space-y-1.5">
          <textarea
            className="w-full border border-ink-200 rounded px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-accent"
            rows={2}
            value={replyBody}
            onChange={(e) => onChangeReply(e.target.value)}
            placeholder="写一条回复…"
            autoFocus
          />
          <div className="flex gap-1.5">
            <button
              type="button"
              disabled={replyPending || !replyBody.trim()}
              className="px-2 py-0.5 rounded bg-ink-900 text-white text-xs disabled:opacity-50"
              onClick={onSubmitReply}
            >
              {replyPending ? '发送中…' : '发送'}
            </button>
            <button
              type="button"
              className="px-2 py-0.5 rounded border border-ink-200 text-xs text-ink-500"
              onClick={onCancelReply}
            >
              取消
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
