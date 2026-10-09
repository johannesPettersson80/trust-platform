// Align and distribute (specification 39, §3.3 item 8): with two or more objects selected, their
// edges or centres line up with the selection's outer edges; with three or more, they spread with
// equal gaps between them. Positions stay whole pixels.

import type { HmiPageModel } from "../drawing/model";
import { drawnBox } from "./editing";

export type AlignEdge = "left" | "centre" | "right" | "top" | "middle" | "bottom";
export type DistributeAxis = "horizontal" | "vertical";

type Box = ReturnType<typeof drawnBox>;

function moved(page: HmiPageModel, deltas: Map<string, [number, number]>): HmiPageModel {
  return {
    ...page,
    object: page.object.map((object) => {
      const delta = deltas.get(object.id);
      return delta
        ? { ...object, at: [Math.round(object.at[0] + delta[0]), Math.round(object.at[1] + delta[1])] }
        : object;
    }),
  };
}

/** Lines up the objects' edges or centres with the selection's outer edges. */
export function alignObjects(page: HmiPageModel, ids: string[], edge: AlignEdge): HmiPageModel {
  const boxes = page.object
    .filter((object) => ids.includes(object.id))
    .map((object) => [object.id, drawnBox(object)] as [string, Box]);
  if (boxes.length < 2) return page;
  const left = Math.min(...boxes.map(([, box]) => box.x));
  const right = Math.max(...boxes.map(([, box]) => box.x + box.width));
  const top = Math.min(...boxes.map(([, box]) => box.y));
  const bottom = Math.max(...boxes.map(([, box]) => box.y + box.height));
  const deltas = new Map<string, [number, number]>();
  for (const [id, box] of boxes) {
    const dx =
      edge === "left"
        ? left - box.x
        : edge === "right"
          ? right - box.x - box.width
          : edge === "centre"
            ? (left + right) / 2 - box.x - box.width / 2
            : 0;
    const dy =
      edge === "top"
        ? top - box.y
        : edge === "bottom"
          ? bottom - box.y - box.height
          : edge === "middle"
            ? (top + bottom) / 2 - box.y - box.height / 2
            : 0;
    deltas.set(id, [dx, dy]);
  }
  return moved(page, deltas);
}

/** Spreads three or more objects with equal gaps, the first and the last staying where they are. */
export function distributeObjects(page: HmiPageModel, ids: string[], axis: DistributeAxis): HmiPageModel {
  const horizontal = axis === "horizontal";
  const boxes = page.object
    .filter((object) => ids.includes(object.id))
    .map((object) => [object.id, drawnBox(object)] as [string, Box])
    .sort(([, a], [, b]) => (horizontal ? a.x - b.x : a.y - b.y));
  if (boxes.length < 3) return page;
  const start = (box: Box): number => (horizontal ? box.x : box.y);
  const length = (box: Box): number => (horizontal ? box.width : box.height);
  const first = boxes[0][1];
  const last = boxes[boxes.length - 1][1];
  const span = start(last) + length(last) - start(first);
  const gap = (span - boxes.reduce((sum, [, box]) => sum + length(box), 0)) / (boxes.length - 1);
  const deltas = new Map<string, [number, number]>();
  let at = start(first);
  for (const [id, box] of boxes) {
    const shift = at - start(box);
    deltas.set(id, horizontal ? [shift, 0] : [0, shift]);
    at += length(box) + gap;
  }
  return moved(page, deltas);
}
