import { Link, NavLink, Outlet } from 'react-router-dom';
import { useLogout, useMe } from '../api/hooks';

const nav = [
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

  return (
    <div className="h-full flex flex-col">
      <header className="border-b border-ink-200 bg-white">
        <div className="max-w-[1400px] mx-auto px-4 h-12 flex items-center gap-6">
          <Link to="/map" className="font-semibold tracking-tight text-ink-950">
            Epistemic
          </Link>
          <nav className="flex gap-1 text-sm">
            {nav.map((n) => (
              <NavLink
                key={n.to}
                to={n.to}
                className={({ isActive }) =>
                  `px-3 py-1.5 rounded-md ${
                    isActive
                      ? 'bg-ink-100 text-ink-950 font-medium'
                      : 'text-ink-600 hover:bg-ink-50'
                  }`
                }
              >
                {n.label}
              </NavLink>
            ))}
          </nav>
          <div className="ml-auto flex items-center gap-3 text-sm text-ink-600">
            {me && <span>{me.name}</span>}
            <button
              className="text-ink-500 hover:text-ink-900"
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
