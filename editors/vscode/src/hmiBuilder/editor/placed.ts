// Faceplates placed on a page in the editor (specification 39, §8.9): a placement takes its
// faceplate's own size with the header above it, stands where it was dropped or beside the symbol
// whose faceplate it is, and shows the values its bound faceplate's objects bind.

import type { HmiObjectModel, HmiPageModel, PlacedFaceplate } from "../drawing/model";
import { PLACED_FACEPLATE, PLACED_HEADER } from "../drawing/placed";
import { nextId, Point, snap } from "./editing";
import type { FaceplateEntry } from "./messages";

/** A placement's size when the faceplate's own size is not known. */
const DEFAULT_SIZE: [number, number] = [400, 300];
/** The space between a symbol and the faceplate placed beside it. */
const BESIDE = 24;

/** A placement's size: the faceplate's own, with the header above it (§8.9 item 4). */
export function placedSize(entry: FaceplateEntry | undefined): [number, number] {
  const [width, height] = entry?.size ?? DEFAULT_SIZE;
  return [width, height + PLACED_HEADER];
}

/** Adds a placed faceplate for an instance, its top-left corner at `at`. */
export function addPlaced(
  page: HmiPageModel,
  faceplate: string,
  instance: string,
  at: Point,
  size: [number, number],
): { page: HmiPageModel; id: string } {
  const id = nextId(page, PLACED_FACEPLATE);
  const created: HmiObjectModel = {
    id,
    type: PLACED_FACEPLATE,
    faceplate,
    ...(instance ? { instance } : {}),
    at: [snap(at[0]), snap(at[1])],
    size,
    rotation: 0,
    bind: {},
  };
  return { page: { ...page, object: [...page.object, created] }, id };
}

/** Where a symbol's faceplate is placed: beside it, to its right. */
export function besideSymbol(symbol: HmiObjectModel): Point {
  return [symbol.at[0] + symbol.size[0] + BESIDE, symbol.at[1]];
}

/** The panel an object opens: its click's, else an equipment object's own (§21.1 item 2). A
 * placed panel opens nothing. */
export function panelOf(object: HmiObjectModel): string | undefined {
  if (object.click?.faceplate) return object.click.faceplate;
  return object.type !== PLACED_FACEPLATE ? object.faceplate : undefined;
}

/** The instance a symbol's faceplate is for: its own, the one its click names, else the instance
 * whose members its bindings name (§8.7 item 4). */
export function symbolInstance(symbol: HmiObjectModel): string | undefined {
  if (symbol.instance) return symbol.instance;
  if (symbol.click?.instance) return symbol.click.instance;
  const parents = Object.values(symbol.bind).map((binding) => binding.path.split(".").slice(0, -1));
  if (parents.length === 0) return undefined;
  const common = parents.reduce((shared, parent) => {
    const kept: string[] = [];
    for (
      let index = 0;
      index < Math.min(shared.length, parent.length) && shared[index].toLowerCase() === parent[index].toLowerCase();
      index += 1
    )
      kept.push(shared[index]);
    return kept;
  });
  return common.length > 0 ? common.join(".") : undefined;
}

/** Every path the placed faceplates' objects bind. */
export function placedPaths(placed: Record<string, PlacedFaceplate | null>): string[] {
  const paths: string[] = [];
  for (const placement of Object.values(placed)) {
    for (const object of placement?.model.object ?? []) {
      for (const binding of Object.values(object.bind))
        for (const path of [binding.path, binding.quality]) if (path && !paths.includes(path)) paths.push(path);
    }
  }
  return paths;
}
