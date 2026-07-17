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
    <div className="h-full flex items-center justify-center bg-surface p-4">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm md-card p-8 space-y-5"
      >
        <div>
          <div className="text-2xl font-medium text-on-surface flex items-center gap-2">
            <span className="inline-block h-2.5 w-2.5 rounded-full bg-primary" />
            Epistemic
          </div>
          <h1 className="text-sm text-on-surface-variant mt-1">接受邀请</h1>
        </div>
        <label className="block text-sm text-on-surface-variant">
          姓名
          <input
            className="md-field w-full mt-1"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
          />
        </label>
        <label className="block text-sm text-on-surface-variant">
          密码（≥8 位）
          <input
            type="password"
            className="md-field w-full mt-1"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            minLength={8}
            required
          />
        </label>
        {error && <p className="text-sm text-error">{error}</p>}
        <button className="md-btn-filled w-full">
          注册并登录
        </button>
      </form>
    </div>
  );
}
