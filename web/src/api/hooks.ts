import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from './client';
import type {
  Annotation,
  EgoResponse,
  ImportBatch,
  MapResponse,
  Project,
  ReadingLevel,
  RelationDetail,
  User,
  WorkCard,
  WorkListItem,
} from './types';

export function useMe() {
  return useQuery({
    queryKey: ['me'],
    queryFn: () => api.get<User>('/auth/me'),
    retry: false,
  });
}

export function useLogin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { email: string; password: string }) =>
      api.post<{ user: User }>('/auth/login', body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['me'] }),
  });
}

export function useLogout() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post('/auth/logout'),
    onSuccess: () => qc.setQueryData(['me'], null),
  });
}

export function useWorks(params: { query?: string; project?: string } = {}) {
  const sp = new URLSearchParams();
  if (params.query) sp.set('query', params.query);
  if (params.project) sp.set('project', params.project);
  const qs = sp.toString();
  return useQuery({
    queryKey: ['works', params],
    queryFn: () => api.get<WorkListItem[]>(`/works${qs ? `?${qs}` : ''}`),
  });
}

export function useWork(id: string | undefined) {
  return useQuery({
    queryKey: ['work', id],
    queryFn: () => api.get<WorkCard>(`/works/${id}`),
    enabled: !!id,
    refetchInterval: 5000,
  });
}

export function useQuickAdd() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: string) => api.post('/works/quick-add', { input }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['works'] }),
  });
}

export function useMap() {
  return useQuery({
    queryKey: ['graph', 'map'],
    queryFn: () => api.get<MapResponse>('/graph/map'),
  });
}

export function useEgo(kind: string, id: string | undefined, depth = 1) {
  return useQuery({
    queryKey: ['graph', 'ego', kind, id, depth],
    queryFn: () => api.get<EgoResponse>(`/graph/ego/${kind}/${id}?depth=${depth}`),
    enabled: !!id,
  });
}

export function useReviewQueue() {
  return useQuery({
    queryKey: ['review-queue'],
    queryFn: () => api.get<RelationDetail[]>('/review-queue'),
  });
}

export function useReviewAction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      verdict,
    }: {
      id: string;
      verdict: 'agree' | 'disagree';
    }) => api.post(`/relations/${id}/review`, { verdict }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['review-queue'] }),
  });
}

export function usePatchRelation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      body,
    }: {
      id: string;
      body: Record<string, unknown>;
    }) => api.patch(`/relations/${id}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['review-queue'] }),
  });
}

export function useSetReading(workId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { status: ReadingLevel; starred?: boolean }) =>
      api.put(`/works/${workId}/reading-status`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['work', workId] }),
  });
}

export function useAnnotations(workId: string | undefined) {
  return useQuery({
    queryKey: ['annotations', workId],
    queryFn: () => api.get<Annotation[]>(`/works/${workId}/annotations`),
    enabled: !!workId,
  });
}

export function useCreateAnnotation(workId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      body: string;
      kind?: string;
      visibility?: string;
    }) => api.post(`/works/${workId}/annotations`, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['annotations', workId] });
      qc.invalidateQueries({ queryKey: ['work', workId] });
    },
  });
}

export function useProjects() {
  return useQuery({
    queryKey: ['projects'],
    queryFn: () => api.get<Project[]>('/projects'),
  });
}

export function useCreateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { name: string; description?: string }) =>
      api.post<Project>('/projects', body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['projects'] }),
  });
}

export function useProjectCoverage(id: string | undefined) {
  return useQuery({
    queryKey: ['project', id, 'coverage'],
    queryFn: () =>
      api.get<{ work_id: string; title: string; readers: { name: string; status: string }[] }[]>(
        `/projects/${id}/coverage`,
      ),
    enabled: !!id,
  });
}

export function useImportPreview() {
  return useMutation({
    mutationFn: (raw_text: string) => api.post<ImportBatch>('/imports', { raw_text }),
  });
}

export function useImportConfirm() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.post(`/imports/${id}/confirm`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['works'] }),
  });
}
