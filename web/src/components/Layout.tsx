import { Link, NavLink, Outlet } from 'react-router-dom';
import { useLogout, useMe } from '../api/hooks';

const nav = [
  { to: '/groups', label: '研究组' },
  { to: '/map', label: '地图' },
  { to: '/papers', label: '论文' },
  { to: '/review', label: '审核队列' },
  { to: '/projects', label: '项目' },
  { to: '/import', label: '导入' },
  { to: '/settings', label: '设置' },
];

export function Layout() {
  const { data: me } = useMe();
  const logout = useLogout();

  const initial = me?.name?.trim()?.[0]?.toUpperCase() ?? '·';

  return (
    <div className="h-full flex flex-col bg-surface">
      <header className="bg-surface-container-low border-b border-outline-variant">
        <div className="max-w-[1400px] mx-auto px-4 h-14 flex items-center gap-6">
          <Link
            to="/groups"
            className="flex items-center gap-2 font-semibold tracking-tight text-on-surface"
          >
            <span className="inline-block h-2.5 w-2.5 rounded-full bg-primary" />
            Epistemic
          </Link>
          <nav className="flex gap-1 text-sm">
            {nav.map((n) => (
              <NavLink
                key={n.to}
                to={n.to}
                className={({ isActive }) =>
                  `px-3 py-1.5 rounded-full transition-colors ${
                    isActive
                      ? 'bg-secondary-container text-on-secondary-container font-medium'
                      : 'text-on-surface-variant hover:bg-surface-container-high'
                  }`
                }
              >
                {n.label}
              </NavLink>
            ))}
          </nav>
          <div className="ml-auto flex items-center gap-3 text-sm">
            {me && (
              <div className="flex items-center gap-2">
                <span
                  className="grid place-items-center h-8 w-8 rounded-full bg-primary-container text-on-primary-container text-xs font-medium"
                  title={me.name}
                >
                  {initial}
                </span>
                <span className="text-on-surface-variant">{me.name}</span>
              </div>
            )}
            <button
              className="md-btn-text md-btn-sm"
              onClick={() => logout.mutate()}
            >
              退出
            </button>
          </div>
        </div>
      </header>
      <main className="flex-1 min-h-0">
        <Outlet />
      </main>
    </div>
  );
}
