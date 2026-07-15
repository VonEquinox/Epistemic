import type { WorkCard } from '../api/types';
import { StatusDot } from './StatusDot';
import { useCreateAnnotation, useSetReading } from '../api/hooks';
import { useState } from 'react';
import type { ReadingLevel } from '../api/types';

const levels: ReadingLevel[] = ['unread', 'skimmed', 'read', 'reproduced', 'needs_review'];

export function PaperCard({ card }: { card: WorkCard }) {
  const v = card.primary_version;
  const setReading = useSetReading(card.work.id);
  const createAnn = useCreateAnnotation(card.work.id);
  const [note, setNote] = useState('');

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
          <ul className="space-y-2">
            {card.claims.map((c) => (
              <li key={c.id} className="border-l-2 border-accent pl-3">
                <p>{c.text}</p>
                <p className="text-xs text-ink-400 mt-0.5">
                  {c.source} · {c.review_status}
                </p>
              </li>
            ))}
          </ul>
        </section>
      )}

      {card.methods.length > 0 && (
        <section>
          <h2 className="font-medium text-ink-800 mb-2">方法</h2>
          <ul className="space-y-2">
            {card.methods.map((m) => (
              <li key={m.id}>
                <span className="font-medium">{m.name}</span>
                {m.description && (
                  <span className="text-ink-600"> — {m.description}</span>
                )}
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
