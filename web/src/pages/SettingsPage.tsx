import { FormEvent, useState } from 'react';
import {
  useCreateMcpToken,
  useInvite,
  useMcpTokens,
  useMe,
  useRevokeMcpToken,
  useUsers,
} from '../api/hooks';
import type { CreatedMcpToken, Invite } from '../api/types';

function roleLabel(role: string) {
  return role === 'admin' ? '管理员' : '成员';
}

export function SettingsPage() {
  const { data: me } = useMe();
  const { data: users, isLoading: usersLoading } = useUsers();
  const invite = useInvite();
  const [email, setEmail] = useState('');
  const [lastInvite, setLastInvite] = useState<Invite | null>(null);
  const [copied, setCopied] = useState(false);
  const { data: mcpTokens } = useMcpTokens();
  const createMcpToken = useCreateMcpToken();
  const revokeMcpToken = useRevokeMcpToken();
  const [mcpTokenName, setMcpTokenName] = useState('Codex');
  const [newMcpToken, setNewMcpToken] = useState<CreatedMcpToken | null>(null);
  const [mcpCopied, setMcpCopied] = useState(false);

  const isAdmin = me?.role === 'admin';
  const invitePath = lastInvite ? `/invite/${lastInvite.token}` : '';
  const inviteUrl =
    lastInvite && typeof window !== 'undefined'
      ? `${window.location.origin}${invitePath}`
      : invitePath;

  const onInvite = (e: FormEvent) => {
    e.preventDefault();
    const trimmed = email.trim();
    if (!trimmed) return;
    invite.mutate(
      { email: trimmed },
      {
        onSuccess: (data) => {
          setLastInvite(data);
          setEmail('');
          setCopied(false);
        },
      },
    );
  };

  const copyLink = async () => {
    if (!inviteUrl) return;
    try {
      await navigator.clipboard.writeText(inviteUrl);
      setCopied(true);
    } catch {
      /* ignore */
    }
  };

  return (
    <div className="max-w-xl mx-auto p-6 space-y-6">
      <h1 className="text-lg font-semibold">设置</h1>

      {me && (
        <section className="border border-ink-200 rounded-lg p-4 bg-white text-sm space-y-1">
          <h2 className="font-medium text-ink-800 mb-2">当前账号</h2>
          <div>
            <span className="text-ink-500">姓名</span> {me.name}
          </div>
          <div>
            <span className="text-ink-500">邮箱</span> {me.email}
          </div>
          <div>
            <span className="text-ink-500">角色</span> {roleLabel(me.role)}
          </div>
        </section>
      )}

      <section className="border border-ink-200 rounded-lg p-4 bg-white space-y-3">
        <div>
          <h2 className="font-medium text-sm text-ink-800">Codex / MCP 访问令牌</h2>
          <p className="mt-1 text-xs text-ink-500">令牌只显示一次，用于让你的 MCP 客户端以当前账号读取有权限的研究图。</p>
        </div>
        <div className="flex gap-2">
          <input
            value={mcpTokenName}
            onChange={(event) => setMcpTokenName(event.target.value)}
            className="flex-1 rounded-md border border-ink-200 px-3 py-2 text-sm"
            placeholder="令牌名称"
          />
          <button
            type="button"
            disabled={!mcpTokenName.trim() || createMcpToken.isPending}
            onClick={() =>
              createMcpToken.mutate(mcpTokenName.trim(), {
                onSuccess: (token) => {
                  setNewMcpToken(token);
                  setMcpCopied(false);
                },
              })
            }
            className="rounded-md bg-ink-900 px-3 py-2 text-sm text-white disabled:opacity-50"
          >
            生成令牌
          </button>
        </div>
        {newMcpToken && (
          <div className="rounded-md border border-amber-200 bg-amber-50 p-3 space-y-2">
            <div className="text-xs text-amber-800">请立即复制，关闭后无法再次查看。</div>
            <code className="block break-all rounded border border-amber-200 bg-white p-2 text-xs">{newMcpToken.token}</code>
            <button
              type="button"
              onClick={async () => {
                await navigator.clipboard.writeText(newMcpToken.token);
                setMcpCopied(true);
              }}
              className="text-xs text-accent hover:underline"
            >
              {mcpCopied ? '已复制' : '复制令牌'}
            </button>
          </div>
        )}
        {mcpTokens && mcpTokens.length > 0 && (
          <ul className="divide-y divide-ink-100 text-sm">
            {mcpTokens.map((token) => (
              <li key={token.id} className="flex items-center justify-between gap-3 py-2">
                <div>
                  <div className="font-medium text-ink-700">{token.name}</div>
                  <div className="text-xs text-ink-400">
                    创建于 {new Date(token.created_at).toLocaleString()}
                    {token.last_used_at ? ` · 最近使用 ${new Date(token.last_used_at).toLocaleString()}` : ''}
                  </div>
                </div>
                <button
                  type="button"
                  disabled={revokeMcpToken.isPending}
                  onClick={() => revokeMcpToken.mutate(token.id)}
                  className="text-xs text-rose-500 hover:text-rose-700"
                >
                  撤销
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {isAdmin && (
        <section className="border border-ink-200 rounded-lg p-4 bg-white space-y-3">
          <h2 className="font-medium text-sm text-ink-800">邀请成员</h2>
          <form className="flex gap-2" onSubmit={onInvite}>
            <input
              type="email"
              className="flex-1 border border-ink-200 rounded-md px-3 py-2 text-sm"
              placeholder="成员邮箱"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
            <button
              type="submit"
              disabled={invite.isPending}
              className="px-3 py-2 rounded-md bg-ink-900 text-white text-sm hover:bg-ink-800 disabled:opacity-50"
            >
              {invite.isPending ? '创建中…' : '生成邀请'}
            </button>
          </form>
          {invite.isError && (
            <p className="text-sm text-rose-600">{(invite.error as Error).message}</p>
          )}
          {lastInvite && (
            <div className="rounded-md bg-ink-50 border border-ink-200 p-3 space-y-2 text-sm">
              <div className="text-ink-600">
                已为 <span className="font-medium text-ink-800">{lastInvite.email}</span>{' '}
                生成邀请链接
              </div>
              <div className="flex items-center gap-2">
                <code className="flex-1 font-mono text-xs break-all bg-white border border-ink-200 rounded px-2 py-1.5">
                  {invitePath}
                </code>
                <button
                  type="button"
                  onClick={copyLink}
                  className="shrink-0 px-2.5 py-1.5 rounded-md border border-ink-200 bg-white text-xs hover:border-accent"
                >
                  {copied ? '已复制' : '复制链接'}
                </button>
              </div>
            </div>
          )}
        </section>
      )}

      <section className="border border-ink-200 rounded-lg p-4 bg-white space-y-3">
        <h2 className="font-medium text-sm text-ink-800">成员列表</h2>
        {usersLoading && <p className="text-sm text-ink-500">加载中…</p>}
        {users && users.length === 0 && (
          <p className="text-sm text-ink-500">暂无成员</p>
        )}
        {users && users.length > 0 && (
          <ul className="divide-y divide-ink-100">
            {users.map((u) => (
              <li key={u.id} className="py-2.5 first:pt-0 last:pb-0 flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-sm font-medium truncate">
                    {u.name || u.email}
                    {me?.id === u.id && (
                      <span className="ml-1.5 text-xs text-ink-400 font-normal">（我）</span>
                    )}
                  </div>
                  <div className="text-xs text-ink-500 truncate">{u.email}</div>
                </div>
                <span
                  className={`shrink-0 text-xs px-2 py-0.5 rounded-full ${
                    u.role === 'admin'
                      ? 'bg-ink-900 text-white'
                      : 'bg-ink-100 text-ink-600'
                  }`}
                >
                  {roleLabel(u.role)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
