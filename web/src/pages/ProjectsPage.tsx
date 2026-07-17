import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useCreateProject, useProjects } from '../api/hooks';

export function ProjectsPage() {
  const { data } = useProjects();
  const create = useCreateProject();
  const [name, setName] = useState('');

  return (
    <div className="max-w-3xl mx-auto p-4 md:p-6 space-y-4">
      <h1 className="text-xl font-medium text-on-surface">项目</h1>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (!name.trim()) return;
          create.mutate({ name }, { onSuccess: () => setName('') });
        }}
      >
        <input
          className="md-field flex-1"
          placeholder="新项目名称"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button className="md-btn-filled">
          创建
        </button>
      </form>
      <ul className="space-y-2">
        {data?.map((p) => (
          <li key={p.id}>
            <Link
              to={`/projects/${p.id}`}
              className="block md-card-outlined p-4 hover:shadow-elev1 transition-shadow"
            >
              <div className="font-medium text-on-surface">{p.name}</div>
              {p.description && (
                <div className="text-sm text-on-surface-variant mt-1">{p.description}</div>
              )}
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}
