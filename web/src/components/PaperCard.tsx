import { useMemo, useState, type ReactNode } from 'react';
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
  useDeleteAnnotation,
  useMe,
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
  queued: 'text-on-surface-variant',
  running: 'text-primary',
  done: 'text-on-surface-variant',
  failed: 'text-error',
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
          className="block text-left text-xs text-primary hover:underline"
        >
          证据 p.{e.page}
          {e.text ? `: “${e.text.slice(0, 60)}${e.text.length > 60 ? '…' : ''}”` : ''}
          {' ↗'}
        </button>
      ))}
    </div>
  );
}

/** Collapsible block used by paper-card major sections / aspect cards. */
function Collapsible({
  title,
  count,
  defaultOpen = true,
  children,
  level = 'section',
}: {
  title: string;
  count?: number;
  defaultOpen?: boolean;
  children: ReactNode;
  level?: 'section' | 'sub';
}) {
  const [open, setOpen] = useState(defaultOpen);
  const isSection = level === 'section';
  return (
    <section className={isSection ? undefined : undefined}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className={`group flex w-full items-center gap-1.5 text-left ${
          isSection
            ? 'border-b border-outline-variant pb-1 mb-2'
            : 'mb-1'
        }`}
      >
        <span
          className={`inline-flex shrink-0 text-on-surface-variant transition-transform duration-150 ${
            open ? 'rotate-90' : ''
          }`}
          aria-hidden
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
            <path d="M4.5 2.5L8 6l-3.5 3.5V2.5z" />
          </svg>
        </span>
        <span
          className={
            isSection
              ? 'text-xs font-medium tracking-wide text-on-surface-variant uppercase'
              : 'text-xs font-medium text-on-surface-variant'
          }
        >
          {title}
        </span>
        {typeof count === 'number' && (
          <span className="text-xs font-normal text-on-surface-variant/80">
            ({count})
          </span>
        )}
        <span className="ml-auto text-[10px] text-on-surface-variant opacity-0 group-hover:opacity-100 transition-opacity">
          {open ? '收起' : '展开'}
        </span>
      </button>
      {open && children}
    </section>
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
        <ul className="space-y-1 text-xs text-on-surface-variant">
          {existing.map((j) => (
            <li key={j.id} className="border border-outline-variant rounded-lg px-2 py-1">
              <span className="font-medium text-on-surface">
                {VERDICT_LABEL[j.verdict] ?? j.verdict}
              </span>
              {j.conditions && (
                <span className="text-on-surface-variant"> · {j.conditions}</span>
              )}
              {j.evidence_url && (
                <a
                  href={j.evidence_url}
                  target="_blank"
                  rel="noreferrer"
                  className="ml-1 text-primary hover:underline"
                >
                  证据链接
                </a>
              )}
              <span className="ml-1 text-on-surface-variant">
                {new Date(j.created_at).toLocaleDateString()}
              </span>
            </li>
          ))}
        </ul>
      )}
      {!open ? (
        <button
          type="button"
          className="text-xs text-primary hover:underline"
          onClick={() => setOpen(true)}
        >
          添加判断
        </button>
      ) : (
        <form
          className="space-y-1.5 md-card-filled p-2"
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
                className={`md-chip ${
                  verdict === v.value ? 'md-chip-selected' : ''
                }`}
              >
                {v.label}
              </button>
            ))}
          </div>
          <input
            className="md-field w-full text-xs"
            placeholder="适用条件（可选）"
            value={conditions}
            onChange={(e) => setConditions(e.target.value)}
          />
          <input
            className="md-field w-full text-xs"
            placeholder="证据 / 实验链接（可选）"
            value={evidenceUrl}
            onChange={(e) => setEvidenceUrl(e.target.value)}
          />
          <div className="flex gap-1.5">
            <button
              type="submit"
              disabled={judge.isPending}
              className="md-btn-filled md-btn-sm"
            >
              {judge.isPending ? '提交中…' : '提交判断'}
            </button>
            <button
              type="button"
              className="md-btn-text md-btn-sm"
              onClick={() => setOpen(false)}
            >
              取消
            </button>
          </div>
          {judge.isError && (
            <p className="text-xs text-error">
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
    <li className="md-card-outlined p-3">
      <p className="text-on-surface">{claim.text}</p>
      <p className="text-xs text-on-surface-variant mt-0.5">
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
  const deleteAnn = useDeleteAnnotation(workId);
  const { data: me } = useMe();
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
        <h1 className="text-xl font-semibold text-on-surface leading-snug">
          {v?.title ?? card.work.title_norm}
        </h1>
        <p className="mt-1 text-on-surface-variant">
          {card.authors.map((a) => a.author.full_name).join(', ')}
        </p>
        <p className="mt-1 text-on-surface-variant">
          {[v?.year, v?.venue_name, v?.arxiv_id && `arXiv:${v.arxiv_id}`]
            .filter(Boolean)
            .join(' · ')}
        </p>
        {v?.pdf_path ? (
          <p className="mt-1 text-xs text-primary">已有 PDF</p>
        ) : (
          <p className="mt-1 text-xs text-on-surface-variant">无 PDF</p>
        )}
        {v?.doi && (
          <p className="mt-0.5 text-xs text-on-surface-variant">DOI: {v.doi}</p>
        )}
        {v?.url && (
          <a
            href={v.url}
            target="_blank"
            rel="noreferrer"
            className="mt-0.5 block text-xs text-primary hover:underline"
          >
            原文链接
          </a>
        )}

        {card.versions.length > 1 && (
          <div className="mt-2">
            <h3 className="text-xs font-medium text-on-surface-variant mb-1">版本家族</h3>
            <ul className="space-y-0.5 text-xs text-on-surface-variant">
              {card.versions.map((ver) => (
                <li key={ver.id} className="flex gap-2 items-baseline">
                  <span className="font-mono text-on-surface-variant">{ver.kind}</span>
                  <span>
                    {[ver.year, ver.venue_name, ver.arxiv_id]
                      .filter(Boolean)
                      .join(' · ') || ver.title.slice(0, 40)}
                  </span>
                  {ver.id === v?.id && (
                    <span className="text-primary">主版本</span>
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
                className="md-chip-static"
                title={p.description}
              >
                {p.name}
              </span>
            ))}
          </div>
        )}
      </header>

      {v?.abstract_text && (
        <Collapsible title="摘要" defaultOpen>
          <p className="text-on-surface leading-relaxed">{v.abstract_text}</p>
        </Collapsible>
      )}

      {card.aspects && card.aspects.length > 0 && (
        <Collapsible
          title="多层分析"
          count={card.aspects.filter(
            (a) =>
              a.summary?.trim() ||
              (Array.isArray(a.bullets) && a.bullets.length > 0),
          ).length}
          defaultOpen
        >
          <div className="space-y-2">
            {card.aspects.map((a) => {
              const bullets = Array.isArray(a.bullets)
                ? (a.bullets as unknown[]).filter(
                    (b): b is string => typeof b === 'string' && b.trim().length > 0,
                  )
                : [];
              if (!a.summary?.trim() && bullets.length === 0) return null;
              return (
                <div key={a.aspect} className="md-card-filled p-3 rounded-xl">
                  <Collapsible
                    title={aspectByKey(a.aspect)?.label ?? a.aspect}
                    level="sub"
                    defaultOpen={false}
                  >
                    {a.summary?.trim() && (
                      <p className="text-on-surface leading-relaxed">{a.summary}</p>
                    )}
                    {bullets.length > 0 && (
                      <ul className="mt-1 list-disc pl-4 text-on-surface-variant space-y-0.5">
                        {bullets.map((b, i) => (
                          <li key={i}>{b}</li>
                        ))}
                      </ul>
                    )}
                    {a.source_text?.trim() && (
                      <p className="mt-1 text-xs text-on-surface-variant italic">
                        “{a.source_text.slice(0, 160)}
                        {a.source_text.length > 160 ? '…' : ''}”
                        {a.page > 0 ? ` · p.${a.page}` : ''}
                      </p>
                    )}
                  </Collapsible>
                </div>
              );
            })}
          </div>
        </Collapsible>
      )}

      {/* ─── 研究 ─── */}
      <Collapsible
        title="研究"
        count={
          dnaFields.length +
          (card.methods.length > 0 ? 1 : 0) +
          (claimsWithJudgments.length > 0 ? claimsWithJudgments.length : 0)
        }
        defaultOpen
      >
        {dnaFields.length === 0 &&
          claimsWithJudgments.length === 0 &&
          card.methods.length === 0 && (
            <p className="text-xs text-on-surface-variant">
              DNA 尚未就绪（抽取完成后会显示研究问题、贡献、方法、结论等）
            </p>
          )}

        {dnaFields.map((f) => (
          <div key={f.key} className="mb-3">
            <Collapsible
              title={f.label}
              count={f.items.length}
              level="sub"
              defaultOpen
            >
              <ul className="space-y-1">
                {f.items.map((e) => (
                  <li key={e.id}>
                    <button
                      type="button"
                      onClick={() => onJumpEvidence?.(e)}
                      className="text-left text-on-surface hover:text-primary"
                    >
                      {e.text}
                    </button>
                    <span className="ml-1 text-xs text-primary">p.{e.page} ↗</span>
                  </li>
                ))}
              </ul>
            </Collapsible>
          </div>
        ))}

        {card.methods.length > 0 && (
          <div className="mb-3">
            <Collapsible
              title="方法与组件"
              count={card.methods.length}
              level="sub"
              defaultOpen
            >
              <ul className="space-y-2">
                {card.methods.map((m) => {
                  const evs = (card.evidence ?? []).filter(
                    (e) => e.extraction_field === `method:${m.name}`,
                  );
                  return (
                    <li key={m.id}>
                      <span className="font-medium text-on-surface">{m.name}</span>
                      {m.description && (
                        <span className="text-on-surface-variant"> — {m.description}</span>
                      )}
                      <EvidenceLinks items={evs} onJump={onJumpEvidence} />
                    </li>
                  );
                })}
              </ul>
            </Collapsible>
          </div>
        )}

        {claimsWithJudgments.length > 0 && (
          <div>
            <Collapsible
              title="主要结论（Claims）"
              count={claimsWithJudgments.length}
              level="sub"
              defaultOpen
            >
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
            </Collapsible>
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
            <Collapsible title="其他字段证据" level="sub" defaultOpen={false}>
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
                        className="text-xs text-left text-on-surface-variant hover:text-primary"
                      >
                        <span className="font-mono text-on-surface-variant">
                          {e.extraction_field}
                        </span>{' '}
                        p.{e.page}: {e.text.slice(0, 60)}
                      </button>
                    </li>
                  ))}
              </ul>
            </Collapsible>
          </div>
        )}
      </Collapsible>

      {/* ─── 图谱 ─── */}
      <Collapsible
        title="图谱"
        count={(card.relations ?? []).filter((r) => r.relation.type !== 'cites').length}
        defaultOpen
      >
        {(card.relations ?? []).filter((r) => r.relation.type !== 'cites')
          .length === 0 ? (
          <p className="text-xs text-on-surface-variant">暂无断言关系</p>
        ) : (
          <div className="space-y-3">
            {GRAPH_GROUPS.map((g) => {
              const items = groupedRelations.get(g.key) ?? [];
              if (items.length === 0) return null;
              return (
                <div key={g.key}>
                  <Collapsible
                    title={g.label}
                    count={items.length}
                    level="sub"
                    defaultOpen
                  >
                    <ul className="space-y-2">
                      {items.map((rd) => (
                        <li
                          key={rd.relation.id}
                          className="md-card-outlined p-2"
                        >
                          <div className="flex flex-wrap items-center gap-2 mb-1">
                            <RelationBadge
                              type={rd.relation.type}
                              status={rd.relation.review_status}
                            />
                            {rd.relation.aspect && (
                              <span className="text-xs text-on-surface-variant">
                                {rd.relation.aspect}
                              </span>
                            )}
                            {rd.relation.confidence != null && (
                              <span className="text-xs text-on-surface-variant">
                                conf {(rd.relation.confidence * 100).toFixed(0)}%
                              </span>
                            )}
                          </div>
                          {rd.relation.explanation && (
                            <p className="text-on-surface text-xs leading-relaxed">
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
                                  className="block text-left text-xs text-primary hover:underline"
                                >
                                  证据 p.{e.page}: “
                                  {e.text.slice(0, 80)}
                                  {e.text.length > 80 ? '…' : ''}”
                                  {' ↗'}
                                </button>
                              ))}
                            </div>
                          )}
                        </li>
                      ))}
                    </ul>
                  </Collapsible>
                </div>
              );
            })}
          </div>
        )}
      </Collapsible>

      {/* ─── 管线 ─── */}
      <Collapsible
        title="管线"
        count={pipeline.length}
        defaultOpen={false}
      >
        {pipeline.length === 0 ? (
          <p className="text-xs text-on-surface-variant">暂无任务记录</p>
        ) : (
          <ul className="space-y-1.5">
            {pipeline.map((job: Job) => (
              <li
                key={job.id}
                className="flex flex-wrap items-start gap-2 text-xs border border-outline-variant rounded-lg px-2 py-1.5"
              >
                <span
                  className={`shrink-0 font-medium ${
                    JOB_STATUS_CLS[job.status] ?? 'text-on-surface-variant'
                  }`}
                >
                  {JOB_STATUS_LABEL[job.status] ?? job.status}
                </span>
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-on-surface">
                    {JOB_KIND_LABEL[job.kind] ?? job.kind}
                  </div>
                  {job.last_error && (
                    <p className="text-error mt-0.5 break-words">
                      {job.last_error}
                    </p>
                  )}
                  <p className="text-on-surface-variant mt-0.5">
                    尝试 {job.attempts} ·{' '}
                    {new Date(job.created_at).toLocaleString()}
                  </p>
                </div>
                {job.status === 'failed' && (
                  <button
                    type="button"
                    disabled={requeue.isPending}
                    className="shrink-0 md-btn-text md-btn-sm"
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
          <p className="mt-1 text-xs text-error">
            重试失败：{(requeue.error as Error).message}
          </p>
        )}
      </Collapsible>

      {/* ─── 团队 ─── */}
      <Collapsible
        title="团队"
        count={
          (annotations?.length ?? card.annotations_count ?? 0) +
          card.reading.length
        }
        defaultOpen
      >
        <div className="mb-3">
          <Collapsible title="阅读状态" level="sub" defaultOpen>
            <div className="flex flex-wrap gap-1.5">
              {levels.map((s) => (
                <button
                  key={s}
                  onClick={() => setReading.mutate({ status: s })}
                  className="md-chip"
                >
                  <StatusDot status={s} />
                </button>
              ))}
            </div>
            {card.reading.length > 0 && (
              <ul className="mt-2 space-y-1 text-on-surface-variant">
                {card.reading.map((r) => (
                  <li key={r.user_id} className="flex items-center gap-1">
                    <StatusDot status={r.status} />
                    {r.starred && <span className="text-primary">★</span>}
                    <span className="text-xs text-on-surface-variant font-mono">
                      {r.user_id.slice(0, 8)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </Collapsible>
        </div>

        <div>
          <Collapsible
            title="批注"
            count={annotations?.length ?? card.annotations_count}
            level="sub"
            defaultOpen
          >
            {annotations && annotations.length > 0 && (
              <ul className="mb-2 space-y-2">
                {annotations.map((a) => (
                  <li
                    key={a.id}
                    className={`bg-surface-container-low rounded-xl p-2 text-xs ${
                      a.parent_id ? 'ml-4 border-l-2 border-outline-variant' : ''
                    }`}
                  >
                    <div className="text-on-surface-variant mb-0.5 flex flex-wrap gap-1 items-center">
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
                      <div className="ml-auto flex items-center gap-2">
                        <button
                          type="button"
                          className="text-primary hover:underline"
                          onClick={() => setReplyTo(a.id)}
                        >
                          回复
                        </button>
                        {me?.id === a.user_id && (
                          <button
                            type="button"
                            className="text-error hover:underline"
                            disabled={deleteAnn.isPending}
                            onClick={() => {
                              if (!window.confirm('删除这条批注？')) return;
                              deleteAnn.mutate(a.id);
                            }}
                          >
                            删除
                          </button>
                        )}
                      </div>
                    </div>
                    <p className="text-on-surface whitespace-pre-wrap">{a.body}</p>
                    {deleteAnn.isError && me?.id === a.user_id && (
                      <p className="mt-1 text-xs text-error">
                        删除失败：{(deleteAnn.error as Error).message}
                      </p>
                    )}
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
                  className="md-field text-xs"
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
                  className="md-field text-xs"
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
                  <span className="inline-flex items-center gap-1 text-xs text-on-surface-variant">
                    回复中
                    <button
                      type="button"
                      className="text-error hover:underline"
                      onClick={() => setReplyTo(null)}
                    >
                      取消
                    </button>
                  </span>
                )}
              </div>
              <div className="flex gap-2">
                <input
                  className="md-field flex-1"
                  placeholder={
                    replyTo ? '写一条回复…' : '写一条批注（笔记 / 猜想 / 问题）…'
                  }
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
                <button
                  type="submit"
                  disabled={createAnn.isPending}
                  className="md-btn-filled md-btn-sm self-center"
                >
                  发送
                </button>
              </div>
            </form>
          </Collapsible>
        </div>
      </Collapsible>

    </div>
  );
}
