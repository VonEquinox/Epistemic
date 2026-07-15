import { useState } from 'react';
import { useImportConfirm, useImportPreview } from '../api/hooks';

export function ImportPage() {
  const [text, setText] = useState('');
  const preview = useImportPreview();
  const confirm = useImportConfirm();
  const batch = preview.data;

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-4">
      <h1 className="text-lg font-semibold">批量导入</h1>
      <p className="text-sm text-ink-500">
        每行一篇：arXiv URL / DOI，或「标题 | URL」。# 开头为注释。
      </p>
      <textarea
        className="w-full h-48 border border-ink-200 rounded-lg p-3 font-mono text-sm"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={`https://arxiv.org/abs/1706.03762\nAttention Is All You Need | https://arxiv.org/pdf/1706.03762.pdf`}
      />
      <div className="flex gap-2">
        <button
          className="px-3 py-2 rounded-md bg-ink-900 text-white text-sm"
          onClick={() => preview.mutate(text)}
          disabled={!text.trim() || preview.isPending}
        >
          解析预览
        </button>
        {batch && (
          <button
            className="px-3 py-2 rounded-md bg-emerald-600 text-white text-sm"
            onClick={() => confirm.mutate(batch.id)}
            disabled={confirm.isPending}
          >
            确认入库
          </button>
        )}
      </div>
      {preview.isError && (
        <p className="text-sm text-rose-600">{(preview.error as Error).message}</p>
      )}
      {batch && (
        <pre className="text-xs bg-ink-50 border border-ink-100 rounded-lg p-3 overflow-auto max-h-80">
          {JSON.stringify(batch.parsed, null, 2)}
        </pre>
      )}
      {confirm.isSuccess && (
        <p className="text-sm text-emerald-700">
          导入完成：{JSON.stringify(confirm.data)}
        </p>
      )}
    </div>
  );
}
