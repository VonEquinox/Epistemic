import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from './client';
import type {
  Annotation,
  AnnotationKind,
  ClaimJudgment,
  ClaimVerdict,
  EgoResponse,
  Graph,
  GraphWithMeta,
  GroupMemberPublic,
  ImportBatch,
  Invite,
  Job,
  MapResponse,
  Project,
  ReadingLevel,
  RelationDetail,
  ResearchGroup,
  ResearchGroupWithMeta,
  User,
  Visibility,
  WorkCard,
  WorkListItem,
  SavedView,
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
    // Poll while any pipeline job is queued/running (DEV.md: 5s, no WebSocket).
    refetchInterval: (q) => {
      const data = q.state.data as WorkCard | undefined;
      const busy = (data?.pipeline ?? []).some(
        (j) => j.status === 'queued' || j.status === 'running',
      );
      return busy ? 5000 : false;
    },
  });
}

export function useQuickAdd() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: string) => api.post('/works/quick-add', { input }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['works'] }),
  });
}

export function useMap(graphId?: string | null) {
  const q = graphId ? `?graph_id=${encodeURIComponent(graphId)}` : '';
  return useQuery({
    queryKey: ['graph', 'map', graphId ?? 'global'],
    queryFn: () => api.get<MapResponse>(`/graph/map${q}`),
  });
}

export function useEgo(
  kind: string,
  id: string | undefined,
  depth = 1,
  mode: 'explore' | 'review' | 'write' = 'explore',
) {
  return useQuery({
    queryKey: ['graph', 'ego', kind, id, depth, mode],
    queryFn: () =>
      api.get<EgoResponse>(
        `/graph/ego/${kind}/${id}?depth=${depth}&mode=${mode}`,
      ),
    enabled: !!id,
  });
}

export function useSavedViews() {
  return useQuery({
    queryKey: ['views'],
    queryFn: () => api.get<SavedView[]>('/views'),
  });
}

export function useCreateSavedView() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      name: string;
      weights: { citation_coupling?: number; method_lineage?: number; topic?: number };
    }) => api.post<SavedView>('/views', body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['views'] }),
  });
}

export function useDeleteSavedView() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.delete(`/views/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['views'] }),
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
      comment,
    }: {
      id: string;
      verdict: 'agree' | 'disagree';
      comment?: string;
    }) =>
      api.post<RelationDetail>(`/relations/${id}/review`, {
        verdict,
        ...(comment != null && comment !== '' ? { comment } : {}),
      }),
    onMutate: async ({ id }) => {
      await qc.cancelQueries({ queryKey: ['review-queue'] });
      const prev = qc.getQueryData<RelationDetail[]>(['review-queue']);
      if (prev) {
        qc.setQueryData<RelationDetail[]>(
          ['review-queue'],
          prev.filter((item) => item.relation.id !== id),
        );
      }
      return { prev };
    },
    onError: (err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(['review-queue'], ctx.prev);
      console.error('[useReviewAction]', err);
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ['review-queue'] });
    },
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
    }) => api.patch<RelationDetail>(`/relations/${id}`, body),
    onMutate: async ({ id, body }) => {
      await qc.cancelQueries({ queryKey: ['review-queue'] });
      const prev = qc.getQueryData<RelationDetail[]>(['review-queue']);
      if (prev) {
        // review_status 改成非 unreviewed → 从队列移除；其余字段就地更新
        const status = body.review_status as string | undefined;
        if (status && status !== 'unreviewed') {
          qc.setQueryData<RelationDetail[]>(
            ['review-queue'],
            prev.filter((item) => item.relation.id !== id),
          );
        } else {
          qc.setQueryData<RelationDetail[]>(
            ['review-queue'],
            prev.map((item) => {
              if (item.relation.id !== id) return item;
              const next = { ...item, relation: { ...item.relation } };
              if (body.relation_type)
                next.relation.type = body.relation_type as typeof next.relation.type;
              if (body.aspect !== undefined)
                next.relation.aspect = body.aspect as string | null;
              if (body.explanation !== undefined)
                next.relation.explanation = body.explanation as string;
              if (body.review_status === 'unreviewed')
                next.relation.review_status = 'unreviewed';
              if (body.swap_direction) {
                next.members = next.members.map((m) => {
                  if (m.role === 'source') return { ...m, role: 'target' };
                  if (m.role === 'target') return { ...m, role: 'source' };
                  return m;
                });
              }
              return next;
            }),
          );
        }
      }
      return { prev };
    },
    onError: (err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(['review-queue'], ctx.prev);
      console.error('[usePatchRelation]', err);
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ['review-queue'] });
    },
  });
}

export function useSetReading(workId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { status: ReadingLevel; starred?: boolean }) =>
      api.put(`/works/${workId}/reading-status`, body),
    onMutate: async (body) => {
      await qc.cancelQueries({ queryKey: ['work', workId] });
      const prev = qc.getQueryData<import('./types').WorkCard>(['work', workId]);
      if (prev) {
        const me = prev.reading[0];
        const nextReading = me
          ? prev.reading.map((r, i) =>
              i === 0
                ? {
                    ...r,
                    status: body.status,
                    starred: body.starred ?? r.starred,
                    updated_at: new Date().toISOString(),
                  }
                : r,
            )
          : [
              {
                user_id: 'optimistic',
                work_id: workId,
                status: body.status,
                starred: body.starred ?? false,
                updated_at: new Date().toISOString(),
              },
            ];
        qc.setQueryData(['work', workId], { ...prev, reading: nextReading });
      }
      return { prev };
    },
    onError: (err, _vars, ctx) => {
      if (ctx?.prev) qc.setQueryData(['work', workId], ctx.prev);
      console.error('[useSetReading]', err);
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ['work', workId] });
    },
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
      kind?: AnnotationKind;
      visibility?: Visibility;
      version_id?: string | null;
      anchor?: {
        page?: number;
        text?: string;
        bbox?: unknown;
      } | unknown;
      parent_id?: string | null;
    }) => api.post<Annotation>(`/works/${workId}/annotations`, body),
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

// ─── Groups + Graphs ─────────────────────────────────────────────────────────

export function useGroups() {
  return useQuery({
    queryKey: ['groups'],
    queryFn: () => api.get<ResearchGroupWithMeta[]>('/groups'),
  });
}

export function useCreateGroup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { name: string; description?: string }) =>
      api.post<ResearchGroup>('/groups', body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['groups'] }),
  });
}

export function useGroup(id: string | undefined) {
  return useQuery({
    queryKey: ['groups', id],
    queryFn: () => api.get<ResearchGroupWithMeta>(`/groups/${id}`),
    enabled: !!id,
  });
}

export function useGroupGraphs(groupId: string | undefined) {
  return useQuery({
    queryKey: ['groups', groupId, 'graphs'],
    queryFn: () => api.get<GraphWithMeta[]>(`/groups/${groupId}/graphs`),
    enabled: !!groupId,
  });
}

export function useCreateGraph(groupId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { name: string; description?: string }) =>
      api.post<Graph>(`/groups/${groupId}/graphs`, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['groups', groupId, 'graphs'] });
      qc.invalidateQueries({ queryKey: ['groups'] });
    },
  });
}

export function useGraph(graphId: string | undefined) {
  return useQuery({
    queryKey: ['graphs', graphId],
    queryFn: () => api.get<GraphWithMeta>(`/groups/graphs/${graphId}`),
    enabled: !!graphId,
  });
}

export function useGroupMembers(groupId: string | undefined) {
  return useQuery({
    queryKey: ['groups', groupId, 'members'],
    queryFn: () => api.get<GroupMemberPublic[]>(`/groups/${groupId}/members`),
    enabled: !!groupId,
  });
}

export function useImportLibraryToGraph(graphId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () =>
      api.post<{ ok: boolean; added: number }>(
        `/groups/graphs/${graphId}/import-library`,
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['graphs', graphId] });
      qc.invalidateQueries({ queryKey: ['graph', 'map', graphId] });
      qc.invalidateQueries({ queryKey: ['groups'] });
    },
  });
}

export function useAddWorksToGraph(graphId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (work_ids: string[]) =>
      api.post<{ ok: boolean; added: number }>(
        `/groups/graphs/${graphId}/works`,
        { work_ids },
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['graphs', graphId] });
      qc.invalidateQueries({ queryKey: ['graph', 'map', graphId] });
    },
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

// ─── M2: evidence & claims ───────────────────────────────────────────────────

export function useWorkEvidence(workId: string | undefined) {
  return useQuery({
    queryKey: ['evidence', 'work', workId],
    queryFn: () => api.get<import('./types').EvidenceSpan[]>(`/works/${workId}/evidence`),
    enabled: !!workId,
  });
}

export function useClaimsFull(workId: string | undefined) {
  return useQuery({
    queryKey: ['claims-full', workId],
    queryFn: () =>
      api.get<import('./types').ClaimWithEvidence[]>(`/works/${workId}/claims-full`),
    enabled: !!workId,
  });
}

export function usePromoteClaim() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      work_id: string;
      version_id: string;
      claim_text: string;
      source_text: string;
      page: number;
      bbox?: unknown;
    }) => api.post('/claims/promote', body),
    onSuccess: (_d, vars) => {
      qc.invalidateQueries({ queryKey: ['work', vars.work_id] });
      qc.invalidateQueries({ queryKey: ['claims-full', vars.work_id] });
      qc.invalidateQueries({ queryKey: ['evidence', 'work', vars.work_id] });
    },
  });
}

export function useClaimJudgment(claimId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      verdict: ClaimVerdict;
      conditions?: string;
      evidence_url?: string;
    }) => api.post<ClaimJudgment>(`/claims/${claimId}/judgments`, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['claims-full'] });
      qc.invalidateQueries({ queryKey: ['work'] });
    },
  });
}

export function useRequeueJob(workId?: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: {
      kind: string;
      version_id?: string;
      work_id?: string;
    }) => api.post<Job>('/jobs/requeue', body),
    onSuccess: () => {
      if (workId) {
        qc.invalidateQueries({ queryKey: ['work', workId] });
      } else {
        qc.invalidateQueries({ queryKey: ['work'] });
      }
    },
  });
}

export function useUsers() {
  return useQuery({
    queryKey: ['users'],
    queryFn: () => api.get<User[]>('/auth/users'),
  });
}

export function useInvite() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { email: string }) =>
      api.post<Invite>('/auth/invites', body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['users'] }),
  });
}

