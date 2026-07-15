import type { EvidenceSpan, WorkCard } from '../api/types';
import { StatusDot } from './StatusDot';
import { useCreateAnnotation, useSetReading } from '../api/hooks';
import { useState } from 'react';
import type { ReadingLevel } from '../api/types';

const levels: ReadingLevel[] = ['unread', 'skimmed', 'read', 'reproduced', 'needs_review'];

export function PaperCard({
  card,
  onJumpEvidence,
}: {
  card: WorkCard;
  onJumpEvidence?: (ev: EvidenceSpan) => void;
}) {
  const v = card.primary_version;
  const setReading = useSetReading(card.work.id);
  const createAnn = useCreateAnnotation(card.work.id);
  const [note, setNote] = useState('');

  const evidenceForClaim = (claimId: string) =>
    (card.evidence ?? []).filter((e) => e.claim_id === claimId);

  return (
    <div className="space-y-6 text-sm">
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
      </header>

      {v?.abstract_text && (
        <section>
          <h2 className="font-medium text-ink-800 mb-1">摘要</h2>
          <p className="text-ink-700 leading-relaxed">{v.abstract_text}</p>
        </section>
      )}

      <section>
        <h2 className="font-medium text-ink-800 mb-2">阅读状态</h2>
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
              <li key={r.user_id}>
                <StatusDot status={r.status} />
                {r.starred && <span className="ml-1 text-amber-500">★</span>}
              </li>
            ))}
          </ul>
        )}
      </section>

      {card.claims.length > 0 && (
        <section>
          <h2 className="font-medium text-ink-800 mb-2">Claims</h2>
          <ul className="space-y-3">
            {card.claims.map((c) => {
              const evs = evidenceForClaim(c.id);
              return (
                <li key={c.id} className="border-l-2 border-accent pl-3">
                  <p>{c.text}</p>
                  <p className="text-xs text-ink-400 mt-0.5">
                    {c.source} · {c.review_status}
                  </p>
                  {evs.map((e) => (
                    <button
                      key={e.id}
                      type="button"
                      onClick={() => onJumpEvidence?.(e)}
                      className="mt-1 block text-left text-xs text-accent hover:underline"
                    >
                      证据 p.{e.page}: “{e.text.slice(0, 80)}
                      {e.text.length > 80 ? '…' : ''}”
                    </button>
                  ))}
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {card.methods.length > 0 && (
        <section>
          <h2 className="font-medium text-ink-800 mb-2">方法</h2>
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
                  {evs.map((e) => (
                    <button
                      key={e.id}
                      type="button"
                      onClick={() => onJumpEvidence?.(e)}
                      className="mt-0.5 block text-left text-xs text-accent hover:underline"
                    >
                      证据 p.{e.page}
                    </button>
                  ))}
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {(card.evidence ?? []).filter(
        (e) => !e.claim_id && e.extraction_field && !e.extraction_field.startsWith('method:'),
      ).length > 0 && (
        <section>
          <h2 className="font-medium text-ink-800 mb-2">字段证据</h2>
          <ul className="space-y-1">
            {(card.evidence ?? [])
              .filter(
                (e) =>
                  !e.claim_id &&
                  e.extraction_field &&
                  !e.extraction_field.startsWith('method:'),
              )
              .map((e) => (
                <li key={e.id}>
                  <button
                    type="button"
                    onClick={() => onJumpEvidence?.(e)}
                    className="text-xs text-left text-ink-600 hover:text-accent"
                  >
                    <span className="font-mono text-ink-400">{e.extraction_field}</span>{' '}
                    p.{e.page}: {e.text.slice(0, 60)}
                  </button>
                </li>
              ))}
          </ul>
        </section>
      )}

      <section>
        <h2 className="font-medium text-ink-800 mb-2">
          批注 ({card.annotations_count})
        </h2>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (!note.trim()) return;
            createAnn.mutate({ body: note, kind: 'note', visibility: 'team' });
            setNote('');
          }}
        >
          <input
            className="flex-1 border border-ink-200 rounded px-2 py-1"
            placeholder="写一条团队批注…"
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <button
            type="submit"
            className="px-3 py-1 rounded bg-ink-900 text-white text-xs"
          >
            发送
          </button>
        </form>
      </section>
    </div>
  );
}
