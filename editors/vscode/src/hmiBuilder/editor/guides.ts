// Guides while dragging (specification 39, §3.3 items 3 and 8): the moving objects snap to the
// edges and centres of the others, and to equal gaps between two of them, with the lines shown;
// where nothing is near they snap to the grid, and with Alt they move freely.

import type { HmiPageModel } from "../drawing/model";
import { drawnBox, Point, snap } from "./editing";

/** A guide line, at `at` on its axis, from `from` to `to` across. */
export interface Guide {
  axis: "x" | "y";
  at: number;
  from: number;
  to: number;
}

/** A marked gap between two edges on one axis, at `at` across. */
export interface Gap {
  axis: "x" | "y";
  from: number;
  to: number;
  at: number;
}

type Box = ReturnType<typeof drawnBox>;

function union(boxes: Box[]): Box {
  const left = Math.min(...boxes.map((box) => box.x));
  const top = Math.min(...boxes.map((box) => box.y));
  const right = Math.max(...boxes.map((box) => box.x + box.width));
  const bottom = Math.max(...boxes.map((box) => box.y + box.height));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** The nearest alignment on one axis within `threshold`: the shift to it, and the line. */
function nearest(
  moving: number[],
  others: Array<[number, Box]>,
  threshold: number,
): { shift: number; at: number; box: Box } | undefined {
  let best: { shift: number; at: number; box: Box } | undefined;
  for (const [at, box] of others) {
    for (const edge of moving) {
      const shift = at - edge;
      if (Math.abs(shift) <= threshold && (!best || Math.abs(shift) < Math.abs(best.shift))) best = { shift, at, box };
    }
  }
  return best;
}

/** The moving box placed with equal gaps between the nearest objects on each side, on one axis. */
function equalGaps(
  moving: Box,
  others: Box[],
  horizontal: boolean,
  threshold: number,
): { shift: number; gaps: Gap[] } | undefined {
  const start = (box: Box): number => (horizontal ? box.x : box.y);
  const end = (box: Box): number => (horizontal ? box.x + box.width : box.y + box.height);
  const across = (box: Box): [number, number] =>
    horizontal ? [box.y, box.y + box.height] : [box.x, box.x + box.width];
  const [low, high] = across(moving);
  const beside = others.filter((box) => {
    const [from, to] = across(box);
    return from < high && low < to;
  });
  const before = beside.filter((box) => end(box) <= start(moving)).sort((a, b) => end(b) - end(a))[0];
  const after = beside.filter((box) => start(box) >= end(moving)).sort((a, b) => start(a) - start(b))[0];
  if (!before || !after) return undefined;
  const size = end(moving) - start(moving);
  const placed = (end(before) + start(after) - size) / 2;
  const shift = Math.round(placed) - start(moving);
  if (Math.abs(shift) > threshold) return undefined;
  const line =
    (Math.max(low, across(before)[0], across(after)[0]) + Math.min(high, across(before)[1], across(after)[1])) / 2;
  const axis = horizontal ? "x" : "y";
  const begin = start(moving) + shift;
  return {
    shift,
    gaps: [
      { axis, from: end(before), to: begin, at: line },
      { axis, from: begin + size, to: start(after), at: line },
    ],
  };
}

/** Moves the objects by `delta`, snapped to the others with guides, else to the grid. */
export function moveWithGuides(
  page: HmiPageModel,
  ids: string[],
  delta: Point,
  free: boolean,
  threshold: number,
): { page: HmiPageModel; guides: Guide[]; gaps: Gap[] } {
  const moving = page.object.filter((object) => ids.includes(object.id));
  const others = page.object.filter((object) => !ids.includes(object.id)).map(drawnBox);
  if (moving.length === 0) return { page, guides: [], gaps: [] };
  const raw = union(moving.map(drawnBox));
  const box: Box = { ...raw, x: raw.x + delta[0], y: raw.y + delta[1] };
  const guides: Guide[] = [];
  const gaps: Gap[] = [];
  const shift: [number | undefined, number | undefined] = [undefined, undefined];
  if (!free) {
    const xs = nearest(
      [box.x, box.x + box.width / 2, box.x + box.width],
      others.flatMap((other) =>
        [other.x, other.x + other.width / 2, other.x + other.width].map((at): [number, Box] => [at, other]),
      ),
      threshold,
    );
    const ys = nearest(
      [box.y, box.y + box.height / 2, box.y + box.height],
      others.flatMap((other) =>
        [other.y, other.y + other.height / 2, other.y + other.height].map((at): [number, Box] => [at, other]),
      ),
      threshold,
    );
    if (xs) {
      shift[0] = xs.shift;
      guides.push({
        axis: "x",
        at: xs.at,
        from: Math.min(box.y, xs.box.y),
        to: Math.max(box.y + box.height, xs.box.y + xs.box.height),
      });
    } else {
      const equal = equalGaps(box, others, true, threshold);
      if (equal) {
        shift[0] = equal.shift;
        gaps.push(...equal.gaps);
      }
    }
    if (ys) {
      shift[1] = ys.shift;
      guides.push({
        axis: "y",
        at: ys.at,
        from: Math.min(box.x, ys.box.x),
        to: Math.max(box.x + box.width, ys.box.x + ys.box.width),
      });
    } else {
      const equal = equalGaps(box, others, false, threshold);
      if (equal) {
        shift[1] = equal.shift;
        gaps.push(...equal.gaps);
      }
    }
  }
  const place = (value: number, axis: 0 | 1): number => {
    const snapped = shift[axis];
    if (snapped !== undefined) return Math.round(value + delta[axis] + snapped);
    return snap(value + delta[axis], free);
  };
  return {
    page: {
      ...page,
      object: page.object.map((object) =>
        ids.includes(object.id) ? { ...object, at: [place(object.at[0], 0), place(object.at[1], 1)] } : object,
      ),
    },
    guides,
    gaps,
  };
}
