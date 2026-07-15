import { useParams } from 'react-router-dom';
import { useAnnotations, useWork } from '../api/hooks';
import { PaperCard } from '../components/PaperCard';

export function PaperDetailPage() {
  const { id } = useParams();
  const { data, isLoading, error } = useWork(id);
  const { data: anns } = useAnnotations(id);

  if (isLoading) return <p className="p-6 text-ink-500">加载中…</p>;
  if (error) return <p className="p-6 text-rose-600">{(error as Error).message}</p>;
  if (!data) return null;

  return (
    <div className="max-w-3xl mx-auto p-6">
      <PaperCard card={data} />
      {anns && anns.length > 0 && (
        <section className="mt-6 space-y-3">
          <h2 className="font-medium text-ink-800">全部批注</h2>
          {anns.map((a) => (
            <div
              key={a.id}
              className="border border-ink-100 rounded-md p-3 text-sm"
            >
              <div className="text-xs text-ink-400 mb-1">
                {a.kind} · {a.visibility} ·{' '}
                {new Date(a.created_at).toLocaleString()}
              </div>
              <p>{a.body}</p>
            </div>
          ))}
        </section>
      )}
    </div>
  );
}
