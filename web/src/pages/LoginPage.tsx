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
    <div className="h-full flex items-center justify-center bg-ink-50">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm bg-white border border-ink-200 rounded-xl p-6 shadow-sm space-y-4"
      >
        <div>
          <h1 className="text-xl font-semibold">Epistemic</h1>
          <p className="text-sm text-ink-500 mt-1">研究组内部论文证据图</p>
        </div>
        <label className="block text-sm">
          <span className="text-ink-600">邮箱</span>
          <input
            type="email"
            className="mt-1 w-full border border-ink-200 rounded-md px-3 py-2"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </label>
        <label className="block text-sm">
          <span className="text-ink-600">密码</span>
          <input
            type="password"
            className="mt-1 w-full border border-ink-200 rounded-md px-3 py-2"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </label>
        {login.isError && (
          <p className="text-sm text-rose-600">{(login.error as Error).message}</p>
        )}
        <button
          type="submit"
          disabled={login.isPending}
          className="w-full bg-ink-900 text-white rounded-md py-2 text-sm font-medium hover:bg-ink-800 disabled:opacity-50"
        >
          {login.isPending ? '登录中…' : '登录'}
        </button>
      </form>
    </div>
  );
}
