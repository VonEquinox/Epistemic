import { Navigate, Route, Routes } from 'react-router-dom';
import { Layout } from './components/Layout';
import { useMe } from './api/hooks';
import { LoginPage } from './pages/LoginPage';
import { MapPage } from './pages/MapPage';
import { PapersPage } from './pages/PapersPage';
import { PaperDetailPage } from './pages/PaperDetailPage';
import { EgoPage } from './pages/EgoPage';
import { ReviewPage } from './pages/ReviewPage';
import { ProjectsPage } from './pages/ProjectsPage';
import { ProjectDetailPage } from './pages/ProjectDetailPage';
import { GroupsPage } from './pages/GroupsPage';
import { GroupDetailPage } from './pages/GroupDetailPage';
import { ImportPage } from './pages/ImportPage';
import { SettingsPage } from './pages/SettingsPage';
import { InvitePage } from './pages/InvitePage';

function RequireAuth({ children }: { children: React.ReactNode }) {
  const { data, isLoading, isError } = useMe();
  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center text-ink-500 text-sm">
        加载中…
      </div>
    );
  }
  if (isError || !data) return <Navigate to="/login" replace />;
  return <>{children}</>;
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/invite/:token" element={<InvitePage />} />
      <Route
        element={
          <RequireAuth>
            <Layout />
          </RequireAuth>
        }
      >
        <Route index element={<Navigate to="/groups" replace />} />
        <Route path="map" element={<MapPage />} />
        <Route path="papers" element={<PapersPage />} />
        <Route path="papers/:id" element={<PaperDetailPage />} />
        <Route path="ego/:kind/:id" element={<EgoPage />} />
        <Route path="review" element={<ReviewPage />} />
        <Route path="groups" element={<GroupsPage />} />
        <Route path="groups/:id" element={<GroupDetailPage />} />
        <Route path="projects" element={<ProjectsPage />} />
        <Route path="projects/:id" element={<ProjectDetailPage />} />
        <Route path="import" element={<ImportPage />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/groups" replace />} />
    </Routes>
  );
}
