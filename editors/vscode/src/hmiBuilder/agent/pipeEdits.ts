// Pipe edits of agent drafts (specification 43, AT-05) with the builder's own pipe editing
// (specification 39, §6): a port counts only when the canvas would offer it, a new pipe is
// joined by the canvas's addPipe, copies come through the builder's copy and paste, and pipes
// leave through deletePipes. A connection the canvas refuses is refused here, by the same rule.

import type { HmiPageModel } from "../drawing/model";
import { portPoint } from "../drawing/pipes";
import type { Catalogue } from "../editor/catalogue";
import { copyText, pasted } from "../editor/clipboard";
import { addPipe, deletePipes } from "../editor/pipes";

/** Each component type's ports as the catalogue names them, which is what the canvas offers. */
export function catalogueTypePorts(catalogue: Catalogue): Record<string, string[]> {
  return Object.fromEntries(catalogue.components.map((component) => [component.type, component.ports ?? []]));
}

/** `object.port` when the canvas would offer that port: the object is on the page, its type's
 * catalogue entry names the port, and the shared drawing places the port on the object. */
export function offeredPort(page: HmiPageModel, value: unknown, ports: Record<string, string[]>): string {
  if (typeof value !== "string") throw new Error("INVALID_ARGUMENT: a port is written object.port.");
  const dot = value.lastIndexOf(".");
  const object = dot > 0 ? page.object.find((candidate) => candidate.id === value.slice(0, dot)) : undefined;
  const port = value.slice(dot + 1);
  if (!object || !(ports[object.type] ?? []).includes(port) || !portPoint(object, port)) {
    throw new Error(
      `INVALID_ARGUMENT: unknown port ${value}; the builder offers only the ports its catalogue names for an object on this page.`,
    );
  }
  return value;
}

/** Why the builder's addPipe left `page` as it was. Its rule (specification 39, §6): a pipe joins
 * two different ports, and only ports that have no pipe yet; a branch goes through a junction. */
export function pipeRefusal(page: HmiPageModel, from: string, to: string): string {
  if (from === to) return `INVALID_ARGUMENT: the builder joins two different ports; ${from} cannot connect to itself.`;
  const taken = [from, to].find((port) => page.pipe.some((pipe) => pipe.from === port || pipe.to === port));
  return taken
    ? `CONFLICT: the builder joins only free ports, and ${taken} already has a pipe; disconnect or reconnect that pipe, or branch through a junction.`
    : `CONFLICT: the builder refuses a pipe from ${from} to ${to}.`;
}

/** A new pipe, joined exactly as the canvas joins one, or refused with the builder's reason. */
export function connectPorts(
  page: HmiPageModel,
  from: unknown,
  to: unknown,
  ports: Record<string, string[]>,
): HmiPageModel {
  const start = offeredPort(page, from, ports),
    end = offeredPort(page, to, ports);
  const joined = addPipe(page, start, end);
  if (joined === page) throw new Error(pipeRefusal(page, start, end));
  return joined;
}

/** The place of the connection the agent names by its two ends. */
export function connectionIndex(page: HmiPageModel, from: unknown, to: unknown): number {
  const index = page.pipe.findIndex((pipe) => pipe.from === from && pipe.to === to);
  if (index < 0) throw new Error(`INVALID_ARGUMENT: unknown connection ${String(from)} -> ${String(to)}.`);
  return index;
}

/** The pipe at `index` moved onto new ports. It keeps its place, binding, style, bends and the
 * instruments attached to it; the new ports must be ones addPipe joins with that pipe gone. */
export function reconnectPipe(
  page: HmiPageModel,
  index: number,
  newFrom: unknown,
  newTo: unknown,
  ports: Record<string, string[]>,
): HmiPageModel {
  const start = offeredPort(page, newFrom, ports),
    end = offeredPort(page, newTo, ports);
  const others = { ...page, pipe: page.pipe.filter((_, at) => at !== index) };
  if (addPipe(others, start, end) === others) throw new Error(pipeRefusal(others, start, end));
  return { ...page, pipe: page.pipe.map((pipe, at) => (at === index ? { ...pipe, from: start, to: end } : pipe)) };
}

/** The named connection removed by the builder's deletePipes, which keeps the attachments of the
 * pipes after it on their own pipes. */
export function disconnectPorts(page: HmiPageModel, from: unknown, to: unknown): HmiPageModel {
  connectionIndex(page, from, to);
  return deletePipes(
    page,
    page.pipe.flatMap((pipe, index) => (pipe.from === from && pipe.to === to ? [`pipe:${index}`] : [])),
  );
}

/** Copies of `ids`, `delta` away, made by the builder's own copy and paste: the pipes between the
 * copies come along and an instrument keeps what it measures only when that came along too.
 * Bindings stay as they are. Each copy then takes the id the agent chose for its original. */
export function duplicateObjects(
  page: HmiPageModel,
  ids: string[],
  newIds: string[],
  delta: [number, number],
): HmiPageModel {
  const text = copyText(page, ids);
  const result = text === undefined ? undefined : pasted(page, text, delta, new Set<string>());
  if (!result) throw new Error("INVALID_ARGUMENT: provide objects on this page to duplicate.");
  // Copy and paste keep the page's order of the originals; name each copy after its original.
  const originals = page.object.filter((object) => ids.includes(object.id)).map((object) => object.id);
  const chosen = new Map(result.ids.map((id, index) => [id, newIds[ids.indexOf(originals[index])]]));
  const rename = (id: string): string => chosen.get(id) ?? id;
  const end = (port: string): string => {
    const dot = port.lastIndexOf(".");
    return dot < 0 ? port : `${rename(port.slice(0, dot))}${port.slice(dot)}`;
  };
  return {
    ...result.page,
    object: result.page.object.map((object) =>
      !chosen.has(object.id)
        ? object
        : {
            ...object,
            id: rename(object.id),
            ...(object.attach !== undefined && chosen.has(object.attach) ? { attach: rename(object.attach) } : {}),
          },
    ),
    pipe: result.page.pipe.map((pipe, index) =>
      index < page.pipe.length ? pipe : { ...pipe, from: end(pipe.from), to: end(pipe.to) },
    ),
  };
}
