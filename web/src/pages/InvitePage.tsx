import { FormEvent, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { api } from '../api/client';
import { useQueryClient } from '@tanstack/react-query';

export function InvitePage() {
  const { token } = useParams();
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const nav = useNavigate();
  const qc = useQueryClient();

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    try {
      await api.post('/auth/register', { token, name, password });
      await qc.invalidateQueries({ queryKey: ['me'] });
      nav('/map');
    } catch (err) {
      setError((err as Error).message);
    }
  };

  return (
    <div className="h-full flex items-center justify-center">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm bg-white border border-ink-200 rounded-xl p-6 space-y-4"
      >
        <h1 className="text-lg font-semibold">接受邀请</h1>
        <label className="block text-sm">
          姓名
          <input
            className="mt-1 w-full border border-ink-200 rounded-md px-3 py-2"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
          />
        </label>
        <label className="block text-sm">
          密码（≥8 位）
          <input
            type="password"
            className="mt-1 w-full border border-ink-200 rounded-md px-3 py-2"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            minLength={8}
            required
          />
        </label>
        {error && <p className="text-sm text-rose-600">{error}</p>}
        <button className="w-full bg-ink-900 text-white rounded-md py-2 text-sm">
          注册并登录
        </button>
      </form>
    </div>
  );
}
