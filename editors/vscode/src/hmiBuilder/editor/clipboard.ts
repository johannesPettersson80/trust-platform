// Copy, paste and duplicate (specification 39, §3.3 item 8). The copied objects travel through the
// clipboard as text, so they paste on any page. A copy keeps every property and binding and gets a
// new id; pipes between copied objects come along, pipes to objects not copied are dropped, and an
// instrument keeps what it measures only when that came along too. A copied binding moves to the
// next number when that variable exists in the program (`TT101` to `TT102`).

import type { HmiBindingModel, HmiObjectModel, HmiPageModel, HmiPipeModel } from "../drawing/model";
import { nextId, Point } from "./editing";
import type { VariableNode } from "./messages";

/** Where a paste lands from the original. */
export const PASTE_STEP: Point = [16, 16];

const MARK = "trust-hmi-objects";

interface Copied {
  [MARK]: 1;
  objects: HmiObjectModel[];
  pipes: HmiPipeModel[];
}

/** The object a port `<object id>.<port>` belongs to. */
function objectOf(port: string): string {
  const dot = port.lastIndexOf(".");
  return dot < 0 ? port : port.slice(0, dot);
}

/** The selected objects, with the pipes between them, as clipboard text; none when nothing is selected. */
export function copyText(page: HmiPageModel, ids: string[]): string | undefined {
  const objects = page.object.filter((object) => ids.includes(object.id));
  if (objects.length === 0) return undefined;
  const copied = new Set(objects.map((object) => object.id));
  const places: number[] = [];
  const pipes = page.pipe.filter((pipe, index) => {
    const kept = copied.has(objectOf(pipe.from)) && copied.has(objectOf(pipe.to));
    if (kept) places.push(index);
    return kept;
  });
  // An instrument on a copied pipe names it by its place among the copied pipes.
  const located = objects.map((object) => {
    if (!object.attach?.startsWith("pipe:")) return object;
    const place = places.indexOf(Number(object.attach.slice("pipe:".length)));
    if (place >= 0) return { ...object, attach: `pipe:${place}` };
    const { attach: _attach, ...free } = object;
    return free;
  });
  const text: Copied = { [MARK]: 1, objects: located, pipes };
  return JSON.stringify(text);
}

/** Every variable path of the program, in lower case. */
export function variablePaths(variables: VariableNode[]): Set<string> {
  const paths = new Set<string>();
  const visit = (nodes: VariableNode[]): void => {
    for (const node of nodes) {
      paths.add(node.path.toLowerCase());
      if (node.children) visit(node.children);
    }
  };
  visit(variables);
  return paths;
}

/** A path with its last number moved on by one, when it has a number: `Main.P101.Running` to `Main.P102.Running`. */
function nextNumbered(path: string): string | undefined {
  const match = /(\d+)(\D*)$/.exec(path);
  if (!match) return undefined;
  const [whole, digits, rest] = match;
  const next = String(Number(digits) + 1).padStart(digits.length, "0");
  return `${path.slice(0, path.length - whole.length)}${next}${rest}`;
}

function renumbered(bind: Record<string, HmiBindingModel>, exists: Set<string>): Record<string, HmiBindingModel> {
  return Object.fromEntries(
    Object.entries(bind).map(([property, binding]) => {
      const next = nextNumbered(binding.path);
      return [property, next !== undefined && exists.has(next.toLowerCase()) ? { ...binding, path: next } : binding];
    }),
  );
}

/** Pastes clipboard text onto the page, `step` from where it was copied; none when the text holds no objects. */
export function pasted(
  page: HmiPageModel,
  text: string,
  step: Point,
  exists: Set<string>,
): { page: HmiPageModel; ids: string[] } | undefined {
  let copied: Copied;
  try {
    copied = JSON.parse(text) as Copied;
  } catch {
    return undefined;
  }
  if (copied?.[MARK] !== 1 || !Array.isArray(copied.objects) || copied.objects.length === 0) return undefined;
  const pipes = Array.isArray(copied.pipes) ? copied.pipes : [];
  const renamed = new Map<string, string>();
  let next = page;
  for (const object of copied.objects) {
    const id = nextId(next, object.type);
    renamed.set(object.id, id);
    const copy: HmiObjectModel = {
      ...object,
      id,
      at: [object.at[0] + step[0], object.at[1] + step[1]],
      bind: renumbered(object.bind ?? {}, exists),
    };
    next = { ...next, object: [...next.object, copy] };
  }
  const ids = [...renamed.values()];
  const port = (name: string): string =>
    `${renamed.get(objectOf(name)) ?? objectOf(name)}${name.slice(objectOf(name).length)}`;
  const copiedPipes: HmiPipeModel[] = pipes.map((pipe) => ({
    ...pipe,
    from: port(pipe.from),
    to: port(pipe.to),
    ...(pipe.waypoints
      ? { waypoints: pipe.waypoints.map(([x, y]): [number, number] => [x + step[0], y + step[1]]) }
      : {}),
    ...(pipe.bind ? { bind: renumbered(pipe.bind, exists) } : {}),
  }));
  // An instrument measures what came along with it, or stands free.
  const firstPipe = next.pipe.length;
  const object = next.object.map((candidate) => {
    if (!ids.includes(candidate.id) || candidate.attach === undefined) return candidate;
    const target = candidate.attach.startsWith("pipe:")
      ? copiedPipeIndex(candidate.attach, copied, firstPipe)
      : renamed.get(candidate.attach);
    if (target !== undefined) return { ...candidate, attach: target };
    const { attach: _attach, ...free } = candidate;
    return free;
  });
  return { page: { ...next, object, pipe: [...next.pipe, ...copiedPipes] }, ids };
}

/** `pipe:<n>`, a place among the copied pipes, as the pasted pipe's place on the page. */
function copiedPipeIndex(attach: string, copied: Copied, firstPipe: number): string | undefined {
  const index = Number(attach.slice("pipe:".length));
  if (!Number.isInteger(index) || index < 0 || index >= copied.pipes.length) return undefined;
  return `pipe:${firstPipe + index}`;
}
