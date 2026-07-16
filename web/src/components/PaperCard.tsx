import { useMemo, useState } from 'react';
import type {
  AnnotationKind,
  Claim,
  ClaimJudgment,
  ClaimVerdict,
  EvidenceSpan,
  Job,
  JobStatus,
  RelationDetail,
  RelationType,
  Visibility,
  WorkCard,
} from '../api/types';
import type { ReadingLevel } from '../api/types';
import {
  useAnnotations,
  useClaimJudgment,
  useClaimsFull,
  useCreateAnnotation,
  useRequeueJob,
  useSetReading,
} from '../api/hooks';
import { RelationBadge } from './RelationBadge';
import { StatusDot } from './StatusDot';
import { aspectByKey } from '../graph/aspects';

const levels: ReadingLevel[] = [
  'unread',
  'skimmed',
  'read',
  'reproduced',
  'needs_review',
];

const VERDICTS: { value: ClaimVerdict; label: string }[] = [
  { value: 'supported', label: '支持' },
  { value: 'partially_supported', label: '部分支持' },
  { value: 'contradicted', label: '反驳' },
  { value: 'not_reproduced', label: '未复现' },
  { value: 'concern', label: '存疑' },
  { value: 'unclear', label: '不清楚' },
];

const VERDICT_LABEL: Record<string, string> = Object.fromEntries(
  VERDICTS.map((v) => [v.value, v.label]),
);

const ANN_KINDS: { value: AnnotationKind; label: string }[] = [
  { value: 'note', label: '笔记' },
  { value: 'conjecture', label: '猜想' },
  { value: 'question', label: '问题' },
];

const VISIBILITIES: { value: Visibility; label: string }[] = [
  { value: 'team', label: '团队' },
  { value: 'private', label: '私人' },
];

const JOB_STATUS_CLS: Record<JobStatus, string> = {
  queued: 'bg-ink-100 text-ink-600',
  running: 'bg-amber-100 text-amber-800',
  done: 'bg-emerald-100 text-emerald-800',
  failed: 'bg-rose-100 text-rose-800',
};

const JOB_STATUS_LABEL: Record<JobStatus, string> = {
  queued: '排队',
  running: '运行中',
  done: '完成',
  failed: '失败',
};

const JOB_KIND_LABEL: Record<string, string> = {
  resolve_metadata: '爬 arXiv HTML / 元数据',
  fetch_pdf: '拉取 PDF（可选）',
  grobid_parse: '解析（已弃用→DNA）',
  extract_dna: 'HTML 全文 → LLM 抽 DNA',
  fetch_references: '获取参考文献（已并入 DNA）',
  update_neighbors_citation: '更新引用邻居',
  update_neighbors_lineage: '更新谱系邻居',
  classify_citation_contexts: '引文上下文分类',
  embed: '向量嵌入',
  propose_pairs: '成对关系候选',
  batch_orch: '批量编排',
};

/** Graph section groups per PRD §5.2 */
type GraphGroupKey =
  | 'prerequisite'
  | 'improves_out'
  | 'improves_in'
  | 'reproduce'
  | 'conflict'
  | 'method'
  | 'other';

const GRAPH_GROUPS: { key: GraphGroupKey; label: string }[] = [
  { key: 'prerequisite', label: '前置' },
  { key: 'improves_out', label: '改进' },
  { key: 'improves_in', label: '被改进' },
  { key: 'reproduce', label: '复现' },
  { key: 'conflict', label: '冲突' },
  { key: 'method', label: '方法' },
  { key: 'other', label: '其他' },
];

function memberRole(
  rd: RelationDetail,
  workId: string,
): 'source' | 'target' | 'unknown' {
  const m = rd.members.find(
    (x) =>
      x.entity_id === workId ||
      x.anchor_work_id === workId,
  );
  if (!m) return 'unknown';
  if (m.role === 'source' || m.role === 'target') return m.role;
  return 'unknown';
}

function classifyRelation(
  rd: RelationDetail,
  workId: string,
): GraphGroupKey {
  const t = rd.relation.type as RelationType;
  const role = memberRole(rd, workId);

  if (t === 'prerequisite_for') return 'prerequisite';
  if (t === 'improves_on') {
    // source improves_on target → out = we improve others; in = others improve us
    if (role === 'source') return 'improves_out';
    if (role === 'target') return 'improves_in';
    return 'improves_out';
  }
  if (t === 'reproduces' || t === 'fails_to_reproduce') return 'reproduce';
  if (t === 'contradicts_claim') return 'conflict';
  if (t === 'uses_method_from' || t === 'alternative_to') return 'method';
  return 'other';
}

function fieldEvidence(
  evidence: EvidenceSpan[] | undefined,
  field: string,
): EvidenceSpan[] {
  return (evidence ?? []).filter((e) => e.extraction_field === field);
}

function EvidenceLinks({
  items,
  onJump,
}: {
  items: EvidenceSpan[];
  onJump?: (ev: EvidenceSpan) => void;
}) {
  if (items.length === 0) return null;
  return (
    <div className="mt-0.5 space-y-0.5">
      {items.map((e) => (
        <button
          key={e.id}
          type="button"
          onClick={() => onJump?.(e)}
          className="block text-left text-xs text-accent hover:underline"
        >
          证据 p.{e.page}
          {e.text ? `: “${e.text.slice(0, 60)}${e.text.length > 60 ? '…' : ''}”` : ''}
        </button>
      ))}
    </div>
  );
}

function ClaimJudgmentForm({
  claimId,
  existing,
}: {
  claimId: string;
  existing?: ClaimJudgment[];
}) {
  const judge = useClaimJudgment(claimId);
  const [verdict, setVerdict] = useState<ClaimVerdict>('supported');
  const [conditions, setConditions] = useState('');
  const [evidenceUrl, setEvidenceUrl] = useState('');
  const [open, setOpen] = useState(false);

  return (
    <div className="mt-2 space-y-1">
      {existing && existing.length > 0 && (
        <ul className="space-y-1 text-xs text-ink-600">
          {existing.map((j) => (
            <li key={j.id} className="border border-ink-100 rounded px-2 py-1">
              <span className="font-medium text-ink-800">
                {VERDICT_LABEL[j.verdict] ?? j.verdict}
              </span>
              {j.conditions && (
                <span className="text-ink-500"> · {j.conditions}</span>
              )}
              {j.evidence_url && (
                <a
                  href={j.evidence_url}
                  target="_blank"
                  rel="noreferrer"
                  className="ml-1 text-accent hover:underline"
                >
                  证据链接
                </a>
              )}
              <span className="ml-1 text-ink-400">
                {new Date(j.created_at).toLocaleDateString()}
              </span>
            </li>
          ))}
        </ul>
      )}
      {!open ? (
        <button
          type="button"
          className="text-xs text-accent hover:underline"
          onClick={() => setOpen(true)}
        >
          添加判断
        </button>
      ) : (
        <form
          className="space-y-1.5 border border-ink-100 rounded-md p-2 bg-ink-50/50"
          onSubmit={(e) => {
            e.preventDefault();
            judge.mutate(
              {
                verdict,
                conditions: conditions || undefined,
                evidence_url: evidenceUrl || undefined,
              },
              {
                onSuccess: () => {
                  setConditions('');
                  setEvidenceUrl('');
                  setOpen(false);
                },
              },
            );
          }}
        >
          <div className="flex flex-wrap gap-1">
            {VERDICTS.map((v) => (
              <button
                key={v.value}
                type="button"
                onClick={() => setVerdict(v.value)}
                className={`px-2 py-0.5 rounded text-xs border ${
                  verdict === v.value
                    ? 'border-accent bg-accent/10 text-accent'
                    : 'border-ink-200 text-ink-600 hover:bg-white'
                }`}
              >
                {v.label}
              </button>
            ))}
          </div>
          <input
            className="w-full border border-ink-200 rounded px-2 py-1 text-xs"
            placeholder="适用条件（可选）"
            value={conditions}
            onChange={(e) => setConditions(e.target.value)}
          />
          <input
            className="w-full border border-ink-200 rounded px-2 py-1 text-xs"
            placeholder="证据 / 实验链接（可选）"
            value={evidenceUrl}
            onChange={(e) => setEvidenceUrl(e.target.value)}
          />
          <div className="flex gap-2">
            <button
              type="submit"
              disabled={judge.isPending}
              className="px-2 py-0.5 rounded bg-ink-900 text-white text-xs disabled:opacity-50"
            >
              {judge.isPending ? '提交中…' : '提交判断'}
            </button>
            <button
              type="button"
              className="px-2 py-0.5 rounded border border-ink-200 text-xs"
              onClick={() => setOpen(false)}
            >
              取消
            </button>
          </div>
          {judge.isError && (
            <p className="text-xs text-rose-600">
              {(judge.error as Error).message}
            </p>
          )}
        </form>
      )}
    </div>
  );
}

function ClaimItem({
  claim,
  evidence,
  onJumpEvidence,
}: {
  claim: Claim;
  evidence: EvidenceSpan[];
  onJumpEvidence?: (ev: EvidenceSpan) => void;
}) {
  return (
    <li className="border-l-2 border-accent pl-3">
      <p>{claim.text}</p>
      <p className="text-xs text-ink-400 mt-0.5">
        {claim.source} · {claim.review_status}
      </p>
      <EvidenceLinks items={evidence} onJump={onJumpEvidence} />
      <ClaimJudgmentForm claimId={claim.id} existing={claim.judgments} />
    </li>
  );
}

export function PaperCard({
  card,
  onJumpEvidence,
}: {
  card: WorkCard;
  onJumpEvidence?: (ev: EvidenceSpan) => void;
}) {
  const v = card.primary_version;
  const workId = card.work.id;
  const setReading = useSetReading(workId);
  const createAnn = useCreateAnnotation(workId);
  const { data: annotations } = useAnnotations(workId);
  const { data: claimsFull } = useClaimsFull(workId);
  const requeue = useRequeueJob(workId);

  const [note, setNote] = useState('');
  const [annKind, setAnnKind] = useState<AnnotationKind>('note');
  const [annVis, setAnnVis] = useState<Visibility>('team');
  const [replyTo, setReplyTo] = useState<string | null>(null);

  const claimsWithJudgments: Claim[] = useMemo(() => {
    if (!claimsFull || claimsFull.length === 0) return card.claims;
    const byId = new Map(claimsFull.map((c) => [c.claim.id, c]));
    return card.claims.map((c) => {
      const full = byId.get(c.id);
      if (!full) return c;
      return { ...c, judgments: full.judgments };
    });
  }, [card.claims, claimsFull]);

  const evidenceForClaim = (claimId: string) => {
    const fromCard = (card.evidence ?? []).filter((e) => e.claim_id === claimId);
    if (fromCard.length > 0) return fromCard;
    const full = claimsFull?.find((c) => c.claim.id === claimId);
    return full?.evidence ?? [];
  };

  const dnaFields = useMemo(() => {
    const fields = [
      { key: 'research_question', label: '研究问题' },
      { key: 'contributions', label: '主要贡献' },
      { key: 'datasets', label: '数据集' },
      { key: 'limitations', label: '局限' },
    ] as const;
    return fields
      .map((f) => ({
        ...f,
        items: fieldEvidence(card.evidence, f.key),
      }))
      .filter((f) => f.items.length > 0);
  }, [card.evidence]);

  const groupedRelations = useMemo(() => {
    const rels = card.relations ?? [];
    const map = new Map<GraphGroupKey, RelationDetail[]>();
    for (const g of GRAPH_GROUPS) map.set(g.key, []);
    for (const rd of rels) {
      if (rd.relation.type === 'cites') continue;
      const key = classifyRelation(rd, workId);
      map.get(key)!.push(rd);
    }
    return map;
  }, [card.relations, workId]);

  const pipeline = card.pipeline ?? [];

  return (
    <div className="space-y-6 text-sm">
      {/* ─── 基本 ─── */}
      <header>
        <h1 className="text-xl font-semibold text-ink-950 leading-snug">
          {v?.title ?? card.work.title_norm}
        </h1>
        <p className="mt-1 text-ink-600">
          {card.authors.map((a) => a.author.full_name).join(', ')}
        </p>
        <p className="mt-1 text-ink-500">
          {[v?.year, v?.venue_name, v?.arxiv_id && `arXiv:${v.arxiv_id}`]
            .filter(Boolean)
            .join(' · ')}
        </p>
        {v?.pdf_path ? (
          <p className="mt-1 text-xs text-emerald-600">已有 PDF</p>
        ) : (
          <p className="mt-1 text-xs text-ink-400">无 PDF</p>
        )}
        {v?.doi && (
          <p className="mt-0.5 text-xs text-ink-500">DOI: {v.doi}</p>
        )}
        {v?.url && (
          <a
            href={v.url}
            target="_blank"
            rel="noreferrer"
            className="mt-0.5 block text-xs text-accent hover:underline"
          >
            原文链接
          </a>
        )}

        {card.versions.length > 1 && (
          <div className="mt-2">
            <h3 className="text-xs font-medium text-ink-500 mb-1">版本家族</h3>
            <ul className="space-y-0.5 text-xs text-ink-600">
              {card.versions.map((ver) => (
                <li key={ver.id} className="flex gap-2 items-baseline">
                  <span className="font-mono text-ink-400">{ver.kind}</span>
                  <span>
                    {[ver.year, ver.venue_name, ver.arxiv_id]
                      .filter(Boolean)
                      .join(' · ') || ver.title.slice(0, 40)}
                  </span>
                  {ver.id === v?.id && (
                    <span className="text-accent">主版本</span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        )}

        {card.projects.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1">
            {card.projects.map((p) => (
              <span
                key={p.id}
                className="inline-flex px-2 py-0.5 rounded-full text-xs bg-ink-100 text-ink-700"
                title={p.description}
              >
                {p.name}
              </span>
            ))}
          </div>
        )}
      </header>

      {v?.abstract_text && (
        <section>
          <h2 className="font-medium text-ink-800 mb-1">摘要</h2>
          <p className="text-ink-700 leading-relaxed">{v.abstract_text}</p>
        </section>
      )}

      {card.aspects && card.aspects.length > 0 && (
        <section>
          <h2 className="font-medium text-ink-800 mb-2">多层分析</h2>
          <div className="space-y-3">
            {card.aspects.map((a) => {
              const bullets = Array.isArray(a.bullets)
                ? (a.bullets as unknown[]).filter(
                    (b): b is string => typeof b === 'string' && b.trim().length > 0,
                  )
                : [];
              if (!a.summary?.trim() && bullets.length === 0) return null;
              return (
                <div key={a.aspect}>
                  <h3 className="text-xs font-medium text-ink-500 mb-1">
                    {aspectByKey(a.aspect)?.label ?? a.aspect}
                  </h3>
                  {a.summary?.trim() && (
                    <p className="text-ink-700 leading-relaxed">{a.summary}</p>
                  )}
                  {bullets.length > 0 && (
                    <ul className="mt-1 list-disc pl-4 text-ink-600 space-y-0.5">
                      {bullets.map((b, i) => (
                        <li key={i}>{b}</li>
                      ))}
                    </ul>
                  )}
                  {a.source_text?.trim() && (
                    <p className="mt-1 text-xs text-ink-400 italic">
                      “{a.source_text.slice(0, 160)}
                      {a.source_text.length > 160 ? '…' : ''}”
                      {a.page > 0 ? ` · p.${a.page}` : ''}
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        </section>
      )}

      {/* ─── 研究 ─── */}
      <section>
        <h2 className="font-medium text-ink-800 mb-2">研究</h2>

        {dnaFields.length === 0 &&
          claimsWithJudgments.length === 0 &&
          card.methods.length === 0 && (
            <p className="text-xs text-ink-400">
              DNA 尚未就绪（抽取完成后会显示研究问题、贡献、方法、结论等）
            </p>
          )}

        {dnaFields.map((f) => (
          <div key={f.key} className="mb-3">
            <h3 className="text-xs font-medium text-ink-500 mb-1">{f.label}</h3>
            <ul className="space-y-1">
              {f.items.map((e) => (
                <li key={e.id}>
                  <button
                    type="button"
                    onClick={() => onJumpEvidence?.(e)}
                    className="text-left text-ink-700 hover:text-accent"
                  >
                    {e.text}
                  </button>
                  <span className="ml-1 text-xs text-ink-400">p.{e.page}</span>
                </li>
              ))}
            </ul>
          </div>
        ))}

        {card.methods.length > 0 && (
          <div className="mb-3">
            <h3 className="text-xs font-medium text-ink-500 mb-1">方法与组件</h3>
            <ul className="space-y-2">
              {card.methods.map((m) => {
                const evs = (card.evidence ?? []).filter(
                  (e) => e.extraction_field === `method:${m.name}`,
                );
                return (
                  <li key={m.id}>
                    <span className="font-medium">{m.name}</span>
                    {m.description && (
                      <span className="text-ink-600"> — {m.description}</span>
                    )}
                    <EvidenceLinks items={evs} onJump={onJumpEvidence} />
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        {claimsWithJudgments.length > 0 && (
          <div>
            <h3 className="text-xs font-medium text-ink-500 mb-1">
              主要结论（Claims）
            </h3>
            <ul className="space-y-3">
              {claimsWithJudgments.map((c) => (
                <ClaimItem
                  key={c.id}
                  claim={c}
                  evidence={evidenceForClaim(c.id)}
                  onJumpEvidence={onJumpEvidence}
                />
              ))}
            </ul>
          </div>
        )}

        {/* leftover field evidence not covered above */}
        {(card.evidence ?? []).filter(
          (e) =>
            !e.claim_id &&
            e.extraction_field &&
            !e.extraction_field.startsWith('method:') &&
            ![
              'research_question',
              'contributions',
              'datasets',
              'limitations',
            ].includes(e.extraction_field),
        ).length > 0 && (
          <div className="mt-3">
            <h3 className="text-xs font-medium text-ink-500 mb-1">其他字段证据</h3>
            <ul className="space-y-1">
              {(card.evidence ?? [])
                .filter(
                  (e) =>
                    !e.claim_id &&
                    e.extraction_field &&
                    !e.extraction_field.startsWith('method:') &&
                    ![
                      'research_question',
                      'contributions',
                      'datasets',
                      'limitations',
                    ].includes(e.extraction_field!),
                )
                .map((e) => (
                  <li key={e.id}>
                    <button
                      type="button"
                      onClick={() => onJumpEvidence?.(e)}
                      className="text-xs text-left text-ink-600 hover:text-accent"
                    >
                      <span className="font-mono text-ink-400">
                        {e.extraction_field}
                      </span>{' '}
                      p.{e.page}: {e.text.slice(0, 60)}
                    </button>
                  </li>
                ))}
            </ul>
          </div>
        )}
      </section>

      {/* ─── 团队 ─── */}
      <section>
        <h2 className="font-medium text-ink-800 mb-2">团队</h2>

        <div className="mb-3">
          <h3 className="text-xs font-medium text-ink-500 mb-1">阅读状态</h3>
          <div className="flex flex-wrap gap-2">
            {levels.map((s) => (
              <button
                key={s}
                onClick={() => setReading.mutate({ status: s })}
                className="px-2 py-1 rounded border border-ink-200 hover:bg-ink-50"
              >
                <StatusDot status={s} />
              </button>
            ))}
          </div>
          {card.reading.length > 0 && (
            <ul className="mt-2 space-y-1 text-ink-600">
              {card.reading.map((r) => (
                <li key={r.user_id} className="flex items-center gap-1">
                  <StatusDot status={r.status} />
                  {r.starred && <span className="text-amber-500">★</span>}
                  <span className="text-xs text-ink-400 font-mono">
                    {r.user_id.slice(0, 8)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div>
          <h3 className="text-xs font-medium text-ink-500 mb-1">
            批注 ({card.annotations_count}
            {annotations ? ` · 已加载 ${annotations.length}` : ''})
          </h3>

          {annotations && annotations.length > 0 && (
            <ul className="mb-2 space-y-2">
              {annotations.map((a) => (
                <li
                  key={a.id}
                  className={`border border-ink-100 rounded-md p-2 text-xs ${
                    a.parent_id ? 'ml-4 border-l-2 border-l-ink-200' : ''
                  }`}
                >
                  <div className="text-ink-400 mb-0.5 flex flex-wrap gap-1 items-center">
                    <span>
                      {ANN_KINDS.find((k) => k.value === a.kind)?.label ?? a.kind}
                    </span>
                    <span>·</span>
                    <span>
                      {VISIBILITIES.find((v) => v.value === a.visibility)
                        ?.label ?? a.visibility}
                    </span>
                    <span>·</span>
                    <span>{new Date(a.created_at).toLocaleString()}</span>
                    <button
                      type="button"
                      className="ml-auto text-accent hover:underline"
                      onClick={() => setReplyTo(a.id)}
                    >
                      回复
                    </button>
                  </div>
                  <p className="text-ink-700 whitespace-pre-wrap">{a.body}</p>
                </li>
              ))}
            </ul>
          )}

          <form
            className="space-y-1.5"
            onSubmit={(e) => {
              e.preventDefault();
              if (!note.trim()) return;
              createAnn.mutate(
                {
                  body: note,
                  kind: annKind,
                  visibility: annVis,
                  parent_id: replyTo,
                },
                {
                  onSuccess: () => {
                    setNote('');
                    setReplyTo(null);
                  },
                },
              );
            }}
          >
            <div className="flex flex-wrap gap-2">
              <select
                className="border border-ink-200 rounded px-2 py-1 text-xs"
                value={annKind}
                onChange={(e) => setAnnKind(e.target.value as AnnotationKind)}
              >
                {ANN_KINDS.map((k) => (
                  <option key={k.value} value={k.value}>
                    {k.label}
                  </option>
                ))}
              </select>
              <select
                className="border border-ink-200 rounded px-2 py-1 text-xs"
                value={annVis}
                onChange={(e) => setAnnVis(e.target.value as Visibility)}
              >
                {VISIBILITIES.map((vis) => (
                  <option key={vis.value} value={vis.value}>
                    {vis.label}
                  </option>
                ))}
              </select>
              {replyTo && (
                <span className="inline-flex items-center gap-1 text-xs text-ink-500">
                  回复中
                  <button
                    type="button"
                    className="text-rose-500 hover:underline"
                    onClick={() => setReplyTo(null)}
                  >
                    取消
                  </button>
                </span>
              )}
            </div>
            <div className="flex gap-2">
              <input
                className="flex-1 border border-ink-200 rounded px-2 py-1"
                placeholder={
                  replyTo ? '写一条回复…' : '写一条批注（笔记 / 猜想 / 问题）…'
                }
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
              <button
                type="submit"
                disabled={createAnn.isPending}
                className="px-3 py-1 rounded bg-ink-900 text-white text-xs disabled:opacity-50"
              >
                发送
              </button>
            </div>
          </form>
        </div>
      </section>

      {/* ─── 图谱 ─── */}
      <section>
        <h2 className="font-medium text-ink-800 mb-2">图谱</h2>
        {(card.relations ?? []).filter((r) => r.relation.type !== 'cites')
          .length === 0 ? (
          <p className="text-xs text-ink-400">暂无断言关系</p>
        ) : (
          <div className="space-y-3">
            {GRAPH_GROUPS.map((g) => {
              const items = groupedRelations.get(g.key) ?? [];
              if (items.length === 0) return null;
              return (
                <div key={g.key}>
                  <h3 className="text-xs font-medium text-ink-500 mb-1">
                    {g.label}
                    <span className="ml-1 text-ink-400 font-normal">
                      ({items.length})
                    </span>
                  </h3>
                  <ul className="space-y-2">
                    {items.map((rd) => (
                      <li
                        key={rd.relation.id}
                        className="border border-ink-100 rounded-md p-2"
                      >
                        <div className="flex flex-wrap items-center gap-2 mb-1">
                          <RelationBadge
                            type={rd.relation.type}
                            status={rd.relation.review_status}
                          />
                          {rd.relation.aspect && (
                            <span className="text-xs text-ink-400">
                              {rd.relation.aspect}
                            </span>
                          )}
                          {rd.relation.confidence != null && (
                            <span className="text-xs text-ink-400">
                              conf {(rd.relation.confidence * 100).toFixed(0)}%
                            </span>
                          )}
                        </div>
                        {rd.relation.explanation && (
                          <p className="text-ink-700 text-xs leading-relaxed">
                            {rd.relation.explanation}
                          </p>
                        )}
                        {rd.evidence.length > 0 && (
                          <div className="mt-1 space-y-0.5">
                            {rd.evidence.map((e) => (
                              <button
                                key={e.id}
                                type="button"
                                onClick={() =>
                                  onJumpEvidence?.({
                                    id: e.id,
                                    version_id: e.version_id,
                                    page: e.page,
                                    text: e.text,
                                    bbox: e.bbox,
                                    created_at: '',
                                    relation_id: rd.relation.id,
                                  })
                                }
                                className="block text-left text-xs text-accent hover:underline"
                              >
                                证据 p.{e.page}: “
                                {e.text.slice(0, 80)}
                                {e.text.length > 80 ? '…' : ''}”
                              </button>
                            ))}
                          </div>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* ─── 管线 ─── */}
      <section>
        <h2 className="font-medium text-ink-800 mb-2">管线</h2>
        {pipeline.length === 0 ? (
          <p className="text-xs text-ink-400">暂无任务记录</p>
        ) : (
          <ul className="space-y-1.5">
            {pipeline.map((job: Job) => (
              <li
                key={job.id}
                className="flex flex-wrap items-start gap-2 text-xs border border-ink-100 rounded px-2 py-1.5"
              >
                <span
                  className={`shrink-0 px-1.5 py-0.5 rounded ${
                    JOB_STATUS_CLS[job.status] ?? 'bg-ink-100 text-ink-600'
                  }`}
                >
                  {JOB_STATUS_LABEL[job.status] ?? job.status}
                </span>
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-ink-800">
                    {JOB_KIND_LABEL[job.kind] ?? job.kind}
                  </div>
                  {job.last_error && (
                    <p className="text-rose-600 mt-0.5 break-words">
                      {job.last_error}
                    </p>
                  )}
                  <p className="text-ink-400 mt-0.5">
                    尝试 {job.attempts} ·{' '}
                    {new Date(job.created_at).toLocaleString()}
                  </p>
                </div>
                {job.status === 'failed' && (
                  <button
                    type="button"
                    disabled={requeue.isPending}
                    className="shrink-0 px-2 py-0.5 rounded border border-ink-200 hover:bg-ink-50 text-ink-700 disabled:opacity-50"
                    onClick={() => {
                      const payload = (job.payload ?? {}) as {
                        version_id?: string;
                        work_id?: string;
                      };
                      requeue.mutate({
                        kind: job.kind,
                        version_id: payload.version_id,
                        work_id: payload.work_id ?? workId,
                      });
                    }}
                  >
                    重试
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
        {requeue.isError && (
          <p className="mt-1 text-xs text-rose-600">
            重试失败：{(requeue.error as Error).message}
          </p>
        )}
      </section>
    </div>
  );
}
