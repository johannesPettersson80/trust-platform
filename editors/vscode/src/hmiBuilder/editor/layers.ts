// Groups and layer order (specification 39, §3.3 item 8). A group is invisible: each member records
// `group = "<name>"`, and selecting a member selects its group. The layer order is the objects'
// order in the page file, the last in front.

import type { HmiPageModel } from "../drawing/model";

export type LayerMove = "forward" | "backward" | "front" | "back";

/** The first free group name of the form `group-<n>`. */
function nextGroup(page: HmiPageModel): string {
  const used = new Set(page.object.map((object) => object.group).filter(Boolean));
  for (let number = 1; ; number += 1) {
    const name = `group-${number}`;
    if (!used.has(name)) return name;
  }
}

/** The selection with the other members of every group it touches. */
export function withGroups(page: HmiPageModel, ids: string[]): string[] {
  const groups = new Set(
    page.object.filter((object) => ids.includes(object.id) && object.group).map((object) => object.group),
  );
  if (groups.size === 0) return ids;
  const members = page.object
    .filter((object) => object.group && groups.has(object.group) && !ids.includes(object.id))
    .map((object) => object.id);
  return [...ids, ...members];
}

/** Groups two or more objects under a new name. */
export function groupObjects(page: HmiPageModel, ids: string[]): HmiPageModel {
  const members = withGroups(page, ids).filter((id) => page.object.some((object) => object.id === id));
  if (members.length < 2) return page;
  const group = nextGroup(page);
  return {
    ...page,
    object: page.object.map((object) => (members.includes(object.id) ? { ...object, group } : object)),
  };
}

/** Ungroups every group the selection touches. */
export function ungroupObjects(page: HmiPageModel, ids: string[]): HmiPageModel {
  const members = withGroups(page, ids);
  return {
    ...page,
    object: page.object.map((object) => {
      if (!members.includes(object.id) || object.group === undefined) return object;
      const { group: _group, ...rest } = object;
      return rest;
    }),
  };
}

/** Moves the selected objects one step, or to the front or the back, keeping their own order. */
export function reorder(page: HmiPageModel, ids: string[], move: LayerMove): HmiPageModel {
  const chosen = page.object.filter((object) => ids.includes(object.id));
  if (chosen.length === 0) return page;
  const rest = page.object.filter((object) => !ids.includes(object.id));
  if (move === "front") return { ...page, object: [...rest, ...chosen] };
  if (move === "back") return { ...page, object: [...chosen, ...rest] };
  const order = [...page.object];
  const forward = move === "forward";
  // One step past the nearest object that is not moved, keeping the moved ones' own order.
  const indices = order.map((object, index) => (ids.includes(object.id) ? index : -1)).filter((index) => index >= 0);
  for (const index of forward ? [...indices].reverse() : indices) {
    const target = forward ? index + 1 : index - 1;
    if (target < 0 || target >= order.length || ids.includes(order[target].id)) continue;
    [order[index], order[target]] = [order[target], order[index]];
  }
  return { ...page, object: order };
}

/** Puts an object just in front of another, as dropping its row onto the other's in the Layers panel. */
export function moveInFrontOf(page: HmiPageModel, id: string, target: string): HmiPageModel {
  const moved = page.object.find((object) => object.id === id);
  if (!moved || id === target) return page;
  const rest = page.object.filter((object) => object.id !== id);
  const at = rest.findIndex((object) => object.id === target);
  if (at < 0) return page;
  return { ...page, object: [...rest.slice(0, at + 1), moved, ...rest.slice(at + 1)] };
}

/** Locks an object against selection and moves on the canvas, or unlocks it. */
export function withLock(page: HmiPageModel, id: string, locked: boolean): HmiPageModel {
  return {
    ...page,
    object: page.object.map((object) => {
      if (object.id !== id) return object;
      if (locked) return { ...object, locked: true };
      const { locked: _locked, ...rest } = object;
      return rest;
    }),
  };
}

/** The objects the canvas lets the user pick: neither locked nor hidden in the editor. */
export function pickable(page: HmiPageModel, hidden: string[]): HmiPageModel {
  return { ...page, object: page.object.filter((object) => !object.locked && !hidden.includes(object.id)) };
}
