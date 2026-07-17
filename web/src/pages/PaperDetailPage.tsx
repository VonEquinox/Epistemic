import { useMemo, useRef, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import {
  useAnnotations,
  useCreateAnnotation,
  useDeleteAnnotation,
  useMe,
  usePromoteClaim,
  useWork,
} from '../api/hooks';
import { PaperCard } from '../components/PaperCard';
import { NodeComments } from '../components/NodeComments';
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
  const graphId = sp.get('graph');
  const { data, isLoading, error } = useWork(id);
  const { data: anns } = useAnnotations(id);
  const pdfRef = useRef<PdfViewerHandle>(null);
  const [activeEv, setActiveEv] = useState<string | null>(sp.get('evidence'));
  const promote = usePromoteClaim();
  const createAnn = useCreateAnnotation(id ?? '');
  const deleteAnn = useDeleteAnnotation(id ?? '');
  const { data: me } = useMe();
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

  if (isLoading) return <p className="p-6 text-on-surface-variant">加载中…</p>;
  if (error) return <p className="p-6 text-error">{(error as Error).message}</p>;
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
      <div className="w-[420px] shrink-0 border-r border-outline-variant overflow-y-auto p-5 bg-surface">
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
          <h2 className="text-xs font-medium tracking-wide text-on-surface-variant uppercase border-b border-outline-variant pb-1">
            PDF 批注{anns ? ` (${anns.length})` : ''}
          </h2>
          {roots.length === 0 && (
            <p className="text-xs text-on-surface-variant">
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
              canDelete={me?.id === a.user_id}
              deletePending={deleteAnn.isPending}
              onDelete={() => {
                if (!window.confirm('删除这条批注？')) return;
                deleteAnn.mutate(a.id);
              }}
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

        <NodeComments graphId={graphId} workId={data.work.id} />
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
  canDelete,
  deletePending,
  onJump,
  onStartReply,
  onCancelReply,
  onChangeReply,
  onSubmitReply,
  onDelete,
}: {
  ann: Annotation;
  replies: Annotation[];
  replying: boolean;
  replyBody: string;
  replyPending: boolean;
  canDelete: boolean;
  deletePending: boolean;
  onJump: () => void;
  onStartReply: () => void;
  onCancelReply: () => void;
  onChangeReply: (v: string) => void;
  onSubmitReply: () => void;
  onDelete: () => void;
}) {
  const anchor = ann.anchor as { page?: number; text?: string } | null | undefined;
  return (
    <div className="bg-surface-container-low rounded-xl p-3 text-sm space-y-2">
      <div className="flex items-center gap-1.5 text-xs text-on-surface-variant flex-wrap">
        <span className="px-1.5 py-0.5 rounded-md bg-surface-container-high text-on-surface-variant">
          {KIND_LABEL[ann.kind] ?? ann.kind}
        </span>
        <span className="px-1.5 py-0.5 rounded-md bg-surface-container text-on-surface-variant">
          {VIS_LABEL[ann.visibility] ?? ann.visibility}
        </span>
        <span className="ml-auto">{new Date(ann.created_at).toLocaleString()}</span>
        {canDelete && (
          <button
            type="button"
            className="text-error hover:underline"
            disabled={deletePending}
            onClick={onDelete}
          >
            删除
          </button>
        )}
      </div>
      <p className="text-on-surface whitespace-pre-wrap">{ann.body}</p>
      {anchor?.page != null && (
        <button
          type="button"
          className="text-xs text-primary hover:underline"
          onClick={onJump}
        >
          定位 p.{anchor.page}
          {anchor.text ? `：${anchor.text.slice(0, 40)}${anchor.text.length > 40 ? '…' : ''}` : ''}
          {' ↗'}
        </button>
      )}
      {replies.length > 0 && (
        <div className="ml-1 border-l-2 border-outline-variant pl-3 space-y-2">
          {replies.map((r) => (
            <div key={r.id} className="text-xs">
              <div className="text-on-surface-variant mb-0.5">
                {KIND_LABEL[r.kind] ?? r.kind} ·{' '}
                {new Date(r.created_at).toLocaleString()}
              </div>
              <p className="text-on-surface whitespace-pre-wrap">{r.body}</p>
            </div>
          ))}
        </div>
      )}
      {!replying ? (
        <button
          type="button"
          className="md-btn-text md-btn-sm"
          onClick={onStartReply}
        >
          回复
        </button>
      ) : (
        <div className="space-y-1.5">
          <textarea
            className="md-field w-full resize-none"
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
              className="md-btn-filled md-btn-sm"
              onClick={onSubmitReply}
            >
              {replyPending ? '发送中…' : '发送'}
            </button>
            <button
              type="button"
              className="md-btn-text md-btn-sm"
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
