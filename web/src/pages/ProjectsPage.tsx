import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useCreateProject, useProjects } from '../api/hooks';

export function ProjectsPage() {
  const { data } = useProjects();
  const create = useCreateProject();
  const [name, setName] = useState('');

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-4">
      <h1 className="text-lg font-semibold">项目</h1>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (!name.trim()) return;
          create.mutate({ name }, { onSuccess: () => setName('') });
        }}
      >
        <input
          className="flex-1 border border-ink-200 rounded-md px-3 py-2 text-sm"
          placeholder="新项目名称"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button className="px-3 py-2 rounded-md bg-ink-900 text-white text-sm">
          创建
        </button>
      </form>
      <ul className="space-y-2">
        {data?.map((p) => (
          <li key={p.id}>
            <Link
              to={`/projects/${p.id}`}
              className="block border border-ink-200 rounded-lg p-4 bg-white hover:border-accent"
            >
              <div className="font-medium">{p.name}</div>
              {p.description && (
                <div className="text-sm text-ink-500 mt-1">{p.description}</div>
              )}
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
