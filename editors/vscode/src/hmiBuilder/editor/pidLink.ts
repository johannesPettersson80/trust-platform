// A page and its P&ID (specification 39, §18 phase 4 item 2): a process object with the same id in
// both is the same object, and its type, variant, tag and bindings, whether it exists, and the
// pipes between such objects are one in both views. Positions, sizes and bends are each view's
// own; texts, lines, displays and controls belong to their view alone.

import type { HmiObjectModel, HmiPageModel, HmiPipeModel } from "../drawing/model";
import { symbolFor } from "../drawing/symbols";

/** Whether an object is a process object, which a page and its P&ID share: equipment, an
 * instrument, a junction or one of the project's own symbols. */
export function isProcessObject(object: HmiObjectModel): boolean {
  return object.type === "junction" || object.type.startsWith("my:") || symbolFor(object.type) !== undefined;
}

/** The target with the source's shared process objects and pipes; nothing when it already has
 * them. An object new to the target comes at its place in the source, scaled from the source's
 * size to the target's. */
export function linkedUpdate(
  source: HmiPageModel,
  sourceSize: [number, number],
  target: HmiPageModel,
  targetSize: [number, number],
): HmiPageModel | undefined {
  const shared = new Map(source.object.filter(isProcessObject).map((object) => [object.id, object]));
  const scale = (at: [number, number]): [number, number] => [
    Math.round((at[0] * targetSize[0]) / sourceSize[0]),
    Math.round((at[1] * targetSize[1]) / sourceSize[1]),
  ];
  const objects: HmiObjectModel[] = [];
  const present = new Set<string>();
  for (const object of target.object) {
    if (!isProcessObject(object)) {
      objects.push(object);
      continue;
    }
    const from = shared.get(object.id);
    // Deleted in the source, it leaves the target.
    if (!from) continue;
    present.add(object.id);
    objects.push(withShared(object, from));
  }
  for (const [id, from] of shared) {
    if (!present.has(id)) objects.push({ ...from, at: scale(from.at) });
  }
  const linked = new Set(objects.filter(isProcessObject).map((object) => object.id));
  const between = (pipe: HmiPipeModel): boolean => linked.has(objectOf(pipe.from)) && linked.has(objectOf(pipe.to));
  const own = target.pipe.filter(between);
  const pipes = [
    ...source.pipe.filter(between).map((pipe) => {
      const { waypoints: _bends, ...shape } = pipe;
      const bends = own.find((candidate) => candidate.from === pipe.from && candidate.to === pipe.to)?.waypoints;
      return bends ? { ...shape, waypoints: bends } : shape;
    }),
    ...target.pipe.filter((pipe) => !between(pipe)),
  ];
  const next: HmiPageModel = { ...target, object: objects, pipe: pipes };
  return same(next, target) ? undefined : next;
}

/** The object as the target draws it, with the source's type, variant, tag and bindings. */
function withShared(object: HmiObjectModel, from: HmiObjectModel): HmiObjectModel {
  const { variant: _variant, tag: _tag, ...placed } = object;
  return {
    ...placed,
    type: from.type,
    bind: from.bind,
    ...(from.variant !== undefined ? { variant: from.variant } : {}),
    ...(from.tag !== undefined ? { tag: from.tag } : {}),
  };
}

/** The object a pipe end names, `<object id>.<port>`. */
function objectOf(end: string): string {
  const dot = end.lastIndexOf(".");
  return dot < 0 ? end : end.slice(0, dot);
}

/** Whether two values are the same, whatever the order of their keys. */
function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const [keysA, keysB] = [
    Object.keys(a).filter((key) => (a as Record<string, unknown>)[key] !== undefined),
    Object.keys(b).filter((key) => (b as Record<string, unknown>)[key] !== undefined),
  ];
  return (
    keysA.length === keysB.length &&
    keysA.every((key) => same((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key]))
  );
}
