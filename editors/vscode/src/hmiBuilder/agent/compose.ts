import { COMPOSITION_PATTERNS, CompositionPattern } from "./patterns";
import { constrainedRegions, layoutOptions, placeRegion, LayoutConstraints, CompositionRole, Region } from "./layout";
import type { HmiPageModel, HmiObjectModel } from "../drawing/model";
import type { Catalogue } from "../editor/catalogue";
import { editAgentPage } from "./edits";
export interface CompositionItem extends Partial<Omit<HmiObjectModel, "at">> {
  id: string;
  type: string;
  region?: CompositionRole;
  pinnedAt?: [number, number];
}
export interface CompositionInput {
  title: string;
  pattern: CompositionPattern["id"];
  layout?: LayoutConstraints;
  size: [number, number];
  items: CompositionItem[];
  connections?: Array<{ from: string; to: string }>;
}
const snap = (value: number): number => Math.floor(value / 8) * 8;
const PATTERNS = COMPOSITION_PATTERNS.map((pattern) => pattern.id);

/** Deterministic initial composition. It never infers process connections or invents a binding.
 * Region proportions give the agent a structured starting point; visual review remains required. */
export function composeAgentPage(input: CompositionInput, catalogue: Catalogue): HmiPageModel {
  if (
    !input ||
    !PATTERNS.includes(input.pattern) ||
    typeof input.title !== "string" ||
    !input.title.trim() ||
    !Array.isArray(input.size) ||
    input.size.length !== 2 ||
    !input.size.every((n) => Number.isInteger(n) && n >= 240 && n <= 4096) ||
    !Array.isArray(input.items) ||
    input.items.length > 500
  )
    throw new Error("INVALID_ARGUMENT: invalid composition constraints.");
  const { gap: GAP, flow } = layoutOptions(input.layout);
  const [width, height] = input.size;
  const ids = input.items.map((item) => item.id);
  if (ids.includes("page-heading") || new Set(ids).size !== ids.length)
    throw new Error("INVALID_ARGUMENT: object ids must be unique and must not use page-heading.");
  const groups = new Map<string, CompositionItem[]>();
  for (const item of input.items) {
    if (!catalogue.components.some((component) => component.type === item.type) && item.type !== "faceplate")
      throw new Error(`INVALID_ARGUMENT: unknown component ${item.type}.`);
    const region =
      item.region ??
      (input.pattern === "settings" ? "controls" : input.pattern === "investigation" ? "trends" : "process");
    if (!["process", "measurements", "trends", "alarms", "controls"].includes(region))
      throw new Error("INVALID_ARGUMENT: unknown region.");
    groups.set(region, [...(groups.get(region) ?? []), item]);
  }
  let areas = new Map<string, Region>();
  const inner: Region = { x: 32, y: 88, width: snap(width - 64), height: snap(height - 120) };
  const bottom = ["trends", "alarms"].filter((name) => groups.has(name));
  const upper = ["process", "measurements", "controls"].filter((name) => groups.has(name));
  const hasBoth = bottom.length > 0 && upper.length > 0;
  const upperHeight = hasBoth ? snap((inner.height - GAP) * 0.6) : inner.height;
  const bottomArea: Region = hasBoth
    ? { ...inner, y: inner.y + upperHeight + GAP, height: inner.height - upperHeight - GAP }
    : inner;
  if (upper.length) {
    const primary = groups.has("process") ? "process" : upper[0];
    const aside = upper.filter((name) => name !== primary);
    const primaryWidth = aside.length ? snap((inner.width - GAP) * 0.67) : inner.width;
    areas.set(primary, { ...inner, width: primaryWidth, height: upperHeight });
    aside.forEach((name, index) => {
      const areaHeight = snap((upperHeight - GAP * (aside.length - 1)) / aside.length);
      areas.set(name, {
        x: inner.x + primaryWidth + GAP,
        y: inner.y + index * (areaHeight + GAP),
        width: inner.width - primaryWidth - GAP,
        height: areaHeight,
      });
    });
  }
  bottom.forEach((name, index) => {
    const areaWidth = snap((bottomArea.width - GAP * (bottom.length - 1)) / bottom.length);
    areas.set(name, { ...bottomArea, x: bottomArea.x + index * (areaWidth + GAP), width: areaWidth });
  });
  areas = constrainedRegions(areas, inner, input.layout);
  let page: HmiPageModel = {
    title: input.title,
    object: [
      {
        id: "page-heading",
        type: "text",
        variant: "heading",
        at: [32, 24],
        size: [snap(width - 64), 40],
        rotation: 0,
        text: input.title,
        bind: {},
      },
    ],
    pipe: [],
  };
  const operations: Array<Record<string, unknown>> = [];
  for (const [role, items] of groups) {
    const outer = areas.get(role)!,
      padding = input.layout?.padding ?? 0;
    const area = {
      x: outer.x + padding,
      y: outer.y + padding,
      width: outer.width - padding * 2,
      height: outer.height - padding * 2,
    };
    if (area.width <= 0 || area.height <= 0) throw new Error(`VALIDATION_FAILED: padding consumes the ${role} region.`);
    const sized = items.map((item) => {
      const definition = catalogue.components.find((component) => component.type === item.type);
      const size = item.size ?? definition?.size;
      if (!size || size.length !== 2 || !size.every((n) => Number.isInteger(n) && n >= 8))
        throw new Error(`INVALID_ARGUMENT: provide a usable size for ${item.id}.`);
      return { item, size };
    });
    if (role === "process" && ["process-overview", "equipment-detail"].includes(input.pattern) && sized.length <= 6) {
      const columns = Math.min(sized.length, 3),
        rows = Math.ceil(sized.length / columns);
      const scale = Math.max(
        1,
        Math.min(
          2.5,
          (area.width - GAP * (columns - 1)) / (columns * Math.max(...sized.map((entry) => entry.size[0]))),
          (area.height - GAP * (rows - 1)) / (rows * Math.max(...sized.map((entry) => entry.size[1]))),
        ),
      );
      for (const entry of sized)
        if (!entry.item.size && !entry.item.pinnedAt)
          entry.size = [snap(entry.size[0] * scale), snap(entry.size[1] * scale)];
    }
    if (role === "measurements" || role === "controls") {
      for (const entry of sized)
        if (!entry.item.size && !entry.item.pinnedAt) entry.size = [snap(area.width), Math.max(80, entry.size[1])];
    }
    const equal = input.layout?.equalSize;
    if (equal && sized.length) {
      const target: [number, number] = [
        Math.max(...sized.map((entry) => entry.size[0])),
        Math.max(...sized.map((entry) => entry.size[1])),
      ];
      for (const entry of sized) {
        const next: [number, number] = [
          equal === "height" ? entry.size[0] : target[0],
          equal === "width" ? entry.size[1] : target[1],
        ];
        if (entry.item.type === "faceplate" && next[0] * entry.size[1] !== next[1] * entry.size[0])
          throw new Error(`VALIDATION_FAILED: equal sizing would distort faceplate ${entry.item.id}.`);
        if ((entry.item.pinnedAt || entry.item.locked) && next.some((n, index) => n !== entry.size[index]))
          throw new Error(`CONFLICT: equal sizing would resize fixed object ${entry.item.id}.`);
        entry.size = next;
      }
    }
    const positions = placeRegion(sized, area, role, GAP, flow);
    sized.forEach(({ item, size }) => {
      const { region: _region, pinnedAt: _pinnedAt, ...properties } = item;
      const at = positions.get(item.id)!;
      operations.push({
        op: "add",
        object: { ...properties, at, size, rotation: item.rotation ?? 0, bind: item.bind ?? {} },
      });
    });
  }
  if (operations.length) page = editAgentPage(page, operations, catalogue);
  if (input.connections?.length)
    page = editAgentPage(
      page,
      input.connections.map((connection) => ({ op: "connect", ...connection })),
      catalogue,
    );
  return page;
}
