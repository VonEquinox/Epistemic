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
        <h2 className="font-medium text-ink-800 text-sm">成员评论</h2>
        <p className="text-xs text-ink-400">请从研究图打开这篇论文，再留下组内评论。</p>
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
      <div className="flex items-center gap-2">
        <h2 className="font-medium text-ink-800 text-sm">
          成员评论{comments ? ` (${comments.length})` : ''}
        </h2>
        {graph && <span className="text-[11px] text-ink-400 truncate">{graph.name}</span>}
      </div>

      <div className="rounded-md border border-ink-200 bg-ink-50 p-3 space-y-2">
        <textarea
          rows={3}
          value={body}
          onChange={(event) => setBody(event.target.value)}
          placeholder="留下评论、idea、思考或 review…"
          className="w-full resize-y rounded border border-ink-200 bg-white px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-accent"
        />
        <div className="flex items-center gap-2">
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value as CommentKind)}
            className="rounded border border-ink-200 bg-white px-2 py-1 text-xs"
          >
            {KIND_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <select
            value={visibility}
            onChange={(event) => setVisibility(event.target.value as Visibility)}
            className="rounded border border-ink-200 bg-white px-2 py-1 text-xs"
          >
            <option value="team">组内可见</option>
            <option value="private">仅自己</option>
          </select>
          <button
            type="button"
            disabled={!body.trim() || createComment.isPending}
            onClick={submit}
            className="ml-auto rounded bg-ink-900 px-3 py-1 text-xs text-white disabled:opacity-50"
          >
            {createComment.isPending ? '发布中…' : '发布'}
          </button>
        </div>
      </div>

      {isLoading && <p className="text-xs text-ink-400">加载评论…</p>}
      {error && <p className="text-xs text-rose-600">{(error as Error).message}</p>}
      {mutationError && <p className="text-xs text-rose-600">{mutationError.message}</p>}
      {!isLoading && roots.length === 0 && (
        <p className="text-xs text-ink-400">这篇论文在当前图中还没有成员评论。</p>
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
    <div className="rounded-md border border-ink-100 p-3 text-sm space-y-2">
      <div className="flex items-center gap-1.5 text-xs text-ink-400">
        <span className="font-medium text-ink-700">{comment.author_name}</span>
        <span className="rounded bg-ink-100 px-1.5 py-0.5 text-ink-600">
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
            className="w-full rounded border border-ink-200 px-2 py-1.5 text-sm"
            autoFocus
          />
          <div className="flex gap-2 text-xs">
            <button disabled={pending || !editingBody.trim()} onClick={() => onSaveEdit(comment)} className="text-accent disabled:opacity-50">保存</button>
            <button onClick={onCancelEdit} className="text-ink-400">取消</button>
          </div>
        </div>
      ) : (
        <p className="whitespace-pre-wrap text-ink-800">{comment.body}</p>
      )}

      {!editing && (
        <div className="flex gap-3 text-xs">
          <button onClick={() => onReply(comment)} className="text-ink-500 hover:text-accent">回复</button>
          {own && <button onClick={() => onEdit(comment)} className="text-ink-500 hover:text-accent">编辑</button>}
          {own && <button onClick={() => onDelete(comment)} className="text-rose-500 hover:text-rose-700">删除</button>}
        </div>
      )}

      {replies.length > 0 && (
        <div className="ml-3 space-y-2 border-l-2 border-ink-100 pl-3">
          {replies.map((reply) => {
            const replyOwn = reply.user_id === myUserId;
            return (
              <div key={reply.id} className="text-xs space-y-1">
                <div className="flex gap-1.5 text-ink-400">
                  <span className="font-medium text-ink-600">{reply.author_name}</span>
                  <span>{new Date(reply.updated_at).toLocaleString()}</span>
                </div>
                {editingId === reply.id ? (
                  <div className="space-y-1">
                    <textarea rows={2} value={editingBody} onChange={(event) => onEditingBody(event.target.value)} className="w-full rounded border border-ink-200 px-2 py-1" />
                    <button disabled={pending || !editingBody.trim()} onClick={() => onSaveEdit(reply)} className="mr-2 text-accent">保存</button>
                    <button onClick={onCancelEdit} className="text-ink-400">取消</button>
                  </div>
                ) : (
                  <p className="whitespace-pre-wrap text-ink-700">{reply.body}</p>
                )}
                {replyOwn && editingId !== reply.id && (
                  <div className="flex gap-2">
                    <button onClick={() => onEdit(reply)} className="text-ink-400 hover:text-accent">编辑</button>
                    <button onClick={() => onDelete(reply)} className="text-rose-400 hover:text-rose-600">删除</button>
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
            className="w-full rounded border border-ink-200 px-2 py-1 text-xs"
            autoFocus
          />
          <div className="flex gap-2 text-xs">
            <button disabled={pending || !replyBody.trim()} onClick={() => onSubmitReply(comment)} className="text-accent disabled:opacity-50">发送</button>
            <button onClick={onCancelReply} className="text-ink-400">取消</button>
          </div>
        </div>
      )}
    </div>
  );
}
