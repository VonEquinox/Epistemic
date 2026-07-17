import { FormEvent, useState } from 'react';
import { Navigate } from 'react-router-dom';
import { useLogin, useMe } from '../api/hooks';

export function LoginPage() {
  const { data: me, isLoading } = useMe();
  const login = useLogin();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');

  if (!isLoading && me) return <Navigate to="/map" replace />;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    login.mutate({ email, password });
  };

  return (
    <div className="h-full flex items-center justify-center bg-surface p-4">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm md-card p-8 space-y-5"
      >
        <div>
          <h1 className="text-2xl font-medium text-on-surface flex items-center gap-2">
            <span className="inline-block h-2.5 w-2.5 rounded-full bg-primary" />
            Epistemic
          </h1>
          <p className="text-sm text-on-surface-variant mt-1">研究组内部论文证据图</p>
        </div>
        <label className="block text-sm space-y-1">
          <span className="text-on-surface-variant">邮箱</span>
          <input
            type="email"
            className="md-field w-full"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </label>
        <label className="block text-sm space-y-1">
          <span className="text-on-surface-variant">密码</span>
          <input
            type="password"
            className="md-field w-full"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </label>
        {login.isError && (
          <p className="text-sm text-error">{(login.error as Error).message}</p>
        )}
        <button
          type="submit"
          disabled={login.isPending}
          className="md-btn-filled w-full"
        >
          {login.isPending ? '登录中…' : '登录'}
        </button>
      </form>
    </div>
  );
}
