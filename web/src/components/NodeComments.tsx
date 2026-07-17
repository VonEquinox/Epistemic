import { useMemo, useState } from 'react';
import {
  useCreateNodeComment,
  useDeleteNodeComment,
  useGraph,
  useMe,
  useNodeComments,
  useUpdateNodeComment,
} from '../api/hooks';
import type { CommentKind, NodeComment, Visibility } from '../api/types';

const KIND_OPTIONS: { value: CommentKind; label: string }[] = [
  { value: 'comment', label: '评论' },
  { value: 'idea', label: 'Idea' },
  { value: 'thinking', label: '思考' },
  { value: 'review', label: 'Review' },
  { value: 'question', label: '问题' },
  { value: 'critique', label: '批评' },
];

const KIND_LABEL = Object.fromEntries(KIND_OPTIONS.map((item) => [item.value, item.label]));

export function NodeComments({ graphId, workId }: { graphId: string | null; workId: string }) {
  const { data: me } = useMe();
  const { data: graph } = useGraph(graphId ?? undefined);
  const { data: comments, isLoading, error } = useNodeComments(graphId, workId);
  const createComment = useCreateNodeComment(graphId, workId);
  const updateComment = useUpdateNodeComment(graphId, workId);
  const deleteComment = useDeleteNodeComment(graphId, workId);
  const [body, setBody] = useState('');
  const [kind, setKind] = useState<CommentKind>('comment');
  const [visibility, setVisibility] = useState<Visibility>('team');
  const [replyingTo, setReplyingTo] = useState<string | null>(null);
  const [replyBody, setReplyBody] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingBody, setEditingBody] = useState('');

  const { roots, children } = useMemo(() => {
    const roots: NodeComment[] = [];
    const children = new Map<string, NodeComment[]>();
    for (const comment of comments ?? []) {
      if (comment.parent_id) {
        const list = children.get(comment.parent_id) ?? [];
        list.push(comment);
        children.set(comment.parent_id, list);
      } else {
        roots.push(comment);
      }
    }
    return { roots, children };
  }, [comments]);

  if (!graphId) {
    return (
      <section className="mt-6 space-y-2">
        <h2 className="text-xs font-medium tracking-wide text-on-surface-variant uppercase border-b border-outline-variant pb-1">成员评论</h2>
        <p className="text-xs text-on-surface-variant">请从研究图打开这篇论文，再留下组内评论。</p>
      </section>
    );
  }

  const submit = async () => {
    if (!body.trim()) return;
    await createComment.mutateAsync({ body: body.trim(), kind, visibility });
    setBody('');
  };

  const submitReply = async (parent: NodeComment) => {
    if (!replyBody.trim()) return;
    await createComment.mutateAsync({
      body: replyBody.trim(),
      kind: 'comment',
      visibility: parent.visibility,
      parent_id: parent.id,
    });
    setReplyingTo(null);
    setReplyBody('');
  };

  const saveEdit = async (comment: NodeComment) => {
    if (!editingBody.trim()) return;
    await updateComment.mutateAsync({ id: comment.id, body: editingBody.trim() });
    setEditingId(null);
    setEditingBody('');
  };

  const remove = async (comment: NodeComment) => {
    if (!window.confirm('删除这条评论？其他成员的回复会保留。')) return;
    await deleteComment.mutateAsync(comment.id);
  };

  const mutationError = createComment.error ?? updateComment.error ?? deleteComment.error;

  return (
    <section className="mt-6 space-y-3">
      <div className="flex items-baseline gap-2 border-b border-outline-variant pb-1">
        <h2 className="text-xs font-medium tracking-wide text-on-surface-variant uppercase">
          成员评论{comments ? ` (${comments.length})` : ''}
        </h2>
        {graph && <span className="text-[11px] text-on-surface-variant truncate">{graph.name}</span>}
      </div>

      <div className="md-card-filled p-3 space-y-2">
        <textarea
          rows={3}
          value={body}
          onChange={(event) => setBody(event.target.value)}
          placeholder="留下评论、idea、思考或 review…"
          className="md-field w-full resize-y"
        />
        <div className="flex flex-wrap items-center gap-1.5">
          {KIND_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => setKind(option.value)}
              className={`md-chip ${kind === option.value ? 'md-chip-selected' : ''}`}
            >
              {option.label}
            </button>
          ))}
          <span className="mx-1 h-4 w-px bg-outline-variant" />
          <button
            type="button"
            onClick={() => setVisibility('team')}
            className={`md-chip ${visibility === 'team' ? 'md-chip-selected' : ''}`}
          >
            组内可见
          </button>
          <button
            type="button"
            onClick={() => setVisibility('private')}
            className={`md-chip ${visibility === 'private' ? 'md-chip-selected' : ''}`}
          >
            仅自己
          </button>
          <button
            type="button"
            disabled={!body.trim() || createComment.isPending}
            onClick={submit}
            className="ml-auto md-btn-filled md-btn-sm"
          >
            {createComment.isPending ? '发布中…' : '发布'}
          </button>
        </div>
      </div>

      {isLoading && <p className="text-xs text-on-surface-variant">加载评论…</p>}
      {error && <p className="text-xs text-error">{(error as Error).message}</p>}
      {mutationError && <p className="text-xs text-error">{mutationError.message}</p>}
      {!isLoading && roots.length === 0 && (
        <p className="text-xs text-on-surface-variant">这篇论文在当前图中还没有成员评论。</p>
      )}

      {roots.map((comment) => (
        <CommentCard
          key={comment.id}
          comment={comment}
          replies={children.get(comment.id) ?? []}
          myUserId={me?.id}
          editingId={editingId}
          editingBody={editingBody}
          replyingTo={replyingTo}
          replyBody={replyBody}
          pending={createComment.isPending || updateComment.isPending || deleteComment.isPending}
          onEdit={(item) => {
            setEditingId(item.id);
            setEditingBody(item.body);
          }}
          onCancelEdit={() => {
            setEditingId(null);
            setEditingBody('');
          }}
          onEditingBody={setEditingBody}
          onSaveEdit={saveEdit}
          onDelete={remove}
          onReply={(item) => {
            setReplyingTo(item.id);
            setReplyBody('');
          }}
          onCancelReply={() => {
            setReplyingTo(null);
            setReplyBody('');
          }}
          onReplyBody={setReplyBody}
          onSubmitReply={submitReply}
        />
      ))}
    </section>
  );
}

function CommentCard({
  comment,
  replies,
  myUserId,
  editingId,
  editingBody,
  replyingTo,
  replyBody,
  pending,
  onEdit,
  onCancelEdit,
  onEditingBody,
  onSaveEdit,
  onDelete,
  onReply,
  onCancelReply,
  onReplyBody,
  onSubmitReply,
}: {
  comment: NodeComment;
  replies: NodeComment[];
  myUserId?: string;
  editingId: string | null;
  editingBody: string;
  replyingTo: string | null;
  replyBody: string;
  pending: boolean;
  onEdit: (comment: NodeComment) => void;
  onCancelEdit: () => void;
  onEditingBody: (body: string) => void;
  onSaveEdit: (comment: NodeComment) => void;
  onDelete: (comment: NodeComment) => void;
  onReply: (comment: NodeComment) => void;
  onCancelReply: () => void;
  onReplyBody: (body: string) => void;
  onSubmitReply: (comment: NodeComment) => void;
}) {
  const own = comment.user_id === myUserId;
  const editing = editingId === comment.id;
  const replying = replyingTo === comment.id;
  const edited = comment.updated_at !== comment.created_at;

  return (
    <div className="bg-surface-container-low rounded-xl p-3 text-sm space-y-2">
      <div className="flex items-center gap-1.5 text-xs text-on-surface-variant">
        <span className="font-medium text-on-surface">{comment.author_name}</span>
        <span className="rounded-md bg-surface-container-high px-1.5 py-0.5 text-on-surface-variant">
          {KIND_LABEL[comment.kind] ?? comment.kind}
        </span>
        {comment.visibility === 'private' && <span>仅自己</span>}
        <span className="ml-auto">
          {new Date(comment.updated_at).toLocaleString()}{edited ? ' · 已编辑' : ''}
        </span>
      </div>

      {editing ? (
        <div className="space-y-1.5">
          <textarea
            rows={3}
            value={editingBody}
            onChange={(event) => onEditingBody(event.target.value)}
            className="md-field w-full resize-y"
            autoFocus
          />
          <div className="flex gap-1.5">
            <button disabled={pending || !editingBody.trim()} onClick={() => onSaveEdit(comment)} className="md-btn-filled md-btn-sm">保存</button>
            <button onClick={onCancelEdit} className="md-btn-text md-btn-sm">取消</button>
          </div>
        </div>
      ) : (
        <p className="whitespace-pre-wrap text-on-surface">{comment.body}</p>
      )}

      {!editing && (
        <div className="flex gap-1">
          <button onClick={() => onReply(comment)} className="md-btn-text md-btn-sm">回复</button>
          {own && <button onClick={() => onEdit(comment)} className="md-btn-text md-btn-sm">编辑</button>}
          {own && <button onClick={() => onDelete(comment)} className="md-btn-text md-btn-sm text-error">删除</button>}
        </div>
      )}

      {replies.length > 0 && (
        <div className="ml-1 space-y-2 border-l-2 border-outline-variant pl-3">
          {replies.map((reply) => {
            const replyOwn = reply.user_id === myUserId;
            return (
              <div key={reply.id} className="text-xs space-y-1">
                <div className="flex gap-1.5 text-on-surface-variant">
                  <span className="font-medium text-on-surface">{reply.author_name}</span>
                  <span>{new Date(reply.updated_at).toLocaleString()}</span>
                </div>
                {editingId === reply.id ? (
                  <div className="space-y-1">
                    <textarea rows={2} value={editingBody} onChange={(event) => onEditingBody(event.target.value)} className="md-field w-full resize-y" />
                    <div className="flex gap-1.5">
                      <button disabled={pending || !editingBody.trim()} onClick={() => onSaveEdit(reply)} className="md-btn-filled md-btn-sm">保存</button>
                      <button onClick={onCancelEdit} className="md-btn-text md-btn-sm">取消</button>
                    </div>
                  </div>
                ) : (
                  <p className="whitespace-pre-wrap text-on-surface-variant">{reply.body}</p>
                )}
                {replyOwn && editingId !== reply.id && (
                  <div className="flex gap-1">
                    <button onClick={() => onEdit(reply)} className="md-btn-text md-btn-sm">编辑</button>
                    <button onClick={() => onDelete(reply)} className="md-btn-text md-btn-sm text-error">删除</button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {replying && (
        <div className="space-y-1.5">
          <textarea
            rows={2}
            value={replyBody}
            onChange={(event) => onReplyBody(event.target.value)}
            placeholder="回复这条评论…"
            className="md-field w-full resize-y"
            autoFocus
          />
          <div className="flex gap-1.5">
            <button disabled={pending || !replyBody.trim()} onClick={() => onSubmitReply(comment)} className="md-btn-filled md-btn-sm">发送</button>
            <button onClick={onCancelReply} className="md-btn-text md-btn-sm">取消</button>
          </div>
        </div>
      )}
    </div>
  );
}
