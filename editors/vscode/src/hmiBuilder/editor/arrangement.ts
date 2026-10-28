// The HMI Builder's panel arrangement (specification 39, §3.2 items 6–7): the side panels' widths
// and whether they are collapsed, and the bottom panel. Remembered for each user; pure functions
// shared by the webview and the tests.

export interface Arrangement {
  left: { width: number; collapsed: boolean };
  right: { width: number; collapsed: boolean };
  bottom: { open: boolean; height: number };
}

/** The first view: Components, canvas and inspector; the bottom panel closed. */
export const DEFAULT_ARRANGEMENT: Arrangement = {
  left: { width: 264, collapsed: false },
  right: { width: 300, collapsed: false },
  bottom: { open: false, height: 220 },
};

/** A collapsed side panel's strip, in pixels. */
export const STRIP = 40;
/** The narrowest canvas beside open side panels. */
export const MIN_CANVAS = 400;
export const LEFT_LIMITS: [number, number] = [200, 480];
export const RIGHT_LIMITS: [number, number] = [240, 520];
export const MIN_BOTTOM = 120;

/** What the editor shows at its size. */
export interface ShownArrangement {
  left: number;
  right: number;
  leftCollapsed: boolean;
  rightCollapsed: boolean;
  bottomHeight: number;
  /** The side panels show as strips because the editor is too narrow. */
  narrow: boolean;
}

const clamp = (value: number, [low, high]: [number, number]): number => Math.min(high, Math.max(low, value));
const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};
const size = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? Math.round(value) : fallback;
const flag = (value: unknown, fallback: boolean): boolean => (typeof value === "boolean" ? value : fallback);

/** A stored arrangement made safe: sizes within their limits; anything unreadable is the first
 * view's. */
export function normalizeArrangement(stored: unknown): Arrangement {
  const all = record(stored);
  const [left, right, bottom] = [record(all.left), record(all.right), record(all.bottom)];
  const first = DEFAULT_ARRANGEMENT;
  return {
    left: {
      width: clamp(size(left.width, first.left.width), LEFT_LIMITS),
      collapsed: flag(left.collapsed, first.left.collapsed),
    },
    right: {
      width: clamp(size(right.width, first.right.width), RIGHT_LIMITS),
      collapsed: flag(right.collapsed, first.right.collapsed),
    },
    bottom: {
      open: flag(bottom.open, first.bottom.open),
      height: Math.max(MIN_BOTTOM, size(bottom.height, first.bottom.height)),
    },
  };
}

/** The bottom panel's height in an editor `height` pixels tall. */
function bottomHeight(wanted: number, height: number): number {
  return Math.max(MIN_BOTTOM, Math.min(Math.floor(height / 2), wanted));
}

/** The arrangement as shown in an editor of `width` × `height`; `peek` is a side panel opened from
 * its strip while the editor is narrow. */
export function shownArrangement(
  arrangement: Arrangement,
  width: number,
  height: number,
  peek?: "left" | "right",
): ShownArrangement {
  const leftOpen = !arrangement.left.collapsed;
  const rightOpen = !arrangement.right.collapsed;
  const used = (leftOpen ? arrangement.left.width : STRIP) + (rightOpen ? arrangement.right.width : STRIP);
  const narrow = (leftOpen || rightOpen) && width - used < MIN_CANVAS;
  const leftCollapsed = narrow ? peek !== "left" : !leftOpen;
  const rightCollapsed = narrow ? peek !== "right" : !rightOpen;
  return {
    left: leftCollapsed ? STRIP : arrangement.left.width,
    right: rightCollapsed ? STRIP : arrangement.right.width,
    leftCollapsed,
    rightCollapsed,
    bottomHeight: bottomHeight(arrangement.bottom.height, height),
    narrow,
  };
}

/** A panel edge dragged by `delta` pixels: the left panel's right edge, the inspector's left edge,
 * or the bottom panel's top edge (from its shown height). */
export function resized(
  arrangement: Arrangement,
  edge: "left" | "right" | "bottom",
  delta: number,
  shownBottom: number,
  height: number,
): Arrangement {
  if (edge === "left")
    return { ...arrangement, left: { ...arrangement.left, width: clamp(arrangement.left.width + delta, LEFT_LIMITS) } };
  if (edge === "right")
    return {
      ...arrangement,
      right: { ...arrangement.right, width: clamp(arrangement.right.width - delta, RIGHT_LIMITS) },
    };
  return { ...arrangement, bottom: { ...arrangement.bottom, height: bottomHeight(shownBottom - delta, height) } };
}
