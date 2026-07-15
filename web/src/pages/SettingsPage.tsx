import { useMe } from '../api/hooks';

export function SettingsPage() {
  const { data: me } = useMe();
  return (
    <div className="max-w-xl mx-auto p-6 space-y-4">
      <h1 className="text-lg font-semibold">设置</h1>
      {me && (
        <div className="border border-ink-200 rounded-lg p-4 bg-white text-sm space-y-1">
          <div>
            <span className="text-ink-500">姓名</span> {me.name}
          </div>
          <div>
            <span className="text-ink-500">邮箱</span> {me.email}
          </div>
          <div>
            <span className="text-ink-500">角色</span> {me.role}
          </div>
        </div>
      )}
      <p className="text-sm text-ink-500">
        邀请成员：admin 调用 <code className="font-mono text-xs">POST /api/v1/auth/invites</code>。
        UI 邀请表单将在后续迭代补齐。
      </p>
    </div>
  );
}
