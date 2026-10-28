/** The first view remains readable; explicit Fit and remembered zoom are unrestricted. */
export const FIRST_VIEW_MIN_ZOOM = 0.75;

/** Whole-page fit with the canvas's 32 px margin on each side. */
export function fittedZoom(size: [number, number], area: [number, number]): number {
  const scale = Math.min((area[0] - 64) / size[0], (area[1] - 64) / size[1]);
  return Math.max(0.1, Math.min(4, Math.round(scale * 100) / 100));
}
