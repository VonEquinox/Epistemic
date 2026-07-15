import * as pdfjs from 'pdfjs-dist';

// Vite-friendly worker
import pdfWorker from 'pdfjs-dist/build/pdf.worker.min.mjs?url';

pdfjs.GlobalWorkerOptions.workerSrc = pdfWorker;

export type PdfDocument = pdfjs.PDFDocumentProxy;

export async function loadPdfFromUrl(url: string): Promise<PdfDocument> {
  const task = pdfjs.getDocument({
    url,
    withCredentials: true,
    // range requests optional; cookie auth works with whole-file fetch
    disableRange: true,
    disableStream: true,
  });
  return task.promise;
}

export async function loadPdfFromBlob(blob: Blob): Promise<PdfDocument> {
  const data = new Uint8Array(await blob.arrayBuffer());
  return pdfjs.getDocument({ data }).promise;
}

/** Fetch authenticated PDF as blob URL. Caller must revokeObjectURL when done. */
export async function fetchPdfBlobUrl(versionId: string): Promise<string> {
  const res = await fetch(`/api/v1/versions/${versionId}/pdf`, {
    credentials: 'include',
  });
  if (!res.ok) {
    throw new Error(`PDF 加载失败: ${res.status}`);
  }
  const blob = await res.blob();
  return URL.createObjectURL(blob);
}

export async function uploadPdf(versionId: string, file: File): Promise<void> {
  const fd = new FormData();
  fd.append('file', file, file.name);
  const res = await fetch(`/api/v1/versions/${versionId}/pdf`, {
    method: 'POST',
    credentials: 'include',
    body: fd,
  });
  if (!res.ok) {
    let msg = res.statusText;
    try {
      const j = await res.json();
      msg = j.error ?? msg;
    } catch {
      /* ignore */
    }
    throw new Error(msg);
  }
}
