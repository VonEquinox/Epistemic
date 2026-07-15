/** PDF-point bbox ↔ viewport CSS rect helpers. */

export interface PdfBBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CssRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function parseBBox(raw: unknown): PdfBBox | null {
  if (!raw) return null;
  if (typeof raw === 'object' && raw !== null) {
    const o = raw as Record<string, unknown>;
    if (
      typeof o.x === 'number' &&
      typeof o.y === 'number' &&
      typeof o.w === 'number' &&
      typeof o.h === 'number'
    ) {
      return { x: o.x, y: o.y, w: o.w, h: o.h };
    }
    // GROBID sometimes uses x0,y0,x1,y1
    if (
      typeof o.x0 === 'number' &&
      typeof o.y0 === 'number' &&
      typeof o.x1 === 'number' &&
      typeof o.y1 === 'number'
    ) {
      return {
        x: o.x0,
        y: o.y0,
        w: o.x1 - o.x0,
        h: o.y1 - o.y0,
      };
    }
  }
  if (Array.isArray(raw) && raw.length >= 4) {
    const [a, b, c, d] = raw.map(Number);
    // could be [x,y,w,h] or [x0,y0,x1,y1]
    if (c > a && d > b && c - a < 2000) {
      // treat as x0 y0 x1 y1 if c,d look like absolute coords
      if (c > 50 || d > 50) {
        return { x: a, y: b, w: c - a, h: d - b };
      }
    }
    return { x: a, y: b, w: c, h: d };
  }
  return null;
}

/** Convert PDF-point bbox to viewport CSS rect given PDF.js viewport. */
export function bboxToViewport(
  bbox: PdfBBox,
  viewport: { convertToViewportRectangle: (r: number[]) => number[] },
): CssRect {
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
