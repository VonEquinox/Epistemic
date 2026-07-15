/** PDF.js helpers — evidence jump & highlight (M2). */

export interface EvidenceBBox {
  page: number;
  /** PDF-point coordinates from GROBID teiCoords, if available */
  bbox?: { x: number; y: number; w: number; h: number };
  text: string;
}

/** Convert PDF-point bbox to viewport CSS rect given PDF.js viewport. */
export function bboxToViewport(
  bbox: { x: number; y: number; w: number; h: number },
  viewport: { convertToViewportRectangle: (r: number[]) => number[] },
): { left: number; top: number; width: number; height: number } {
  const [x1, y1, x2, y2] = viewport.convertToViewportRectangle([
    bbox.x,
    bbox.y,
    bbox.x + bbox.w,
    bbox.y + bbox.h,
  ]);
  return {
    left: Math.min(x1, x2),
    top: Math.min(y1, y2),
    width: Math.abs(x2 - x1),
    height: Math.abs(y2 - y1),
  };
}
