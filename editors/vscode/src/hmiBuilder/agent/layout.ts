import { drawnBox } from "../editor/editing";
import type { HmiObjectModel } from "../drawing/model";

export type CompositionRole = "process" | "measurements" | "trends" | "alarms" | "controls";
export interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface LayoutConstraints {
  regions?: Partial<Record<CompositionRole, Region>>;
  gap?: number;
  padding?: number;
  equalSize?: "width" | "height" | "both";
  flow?: "horizontal" | "vertical";
}
export interface SizedItem {
  item: Partial<HmiObjectModel> & { id: string; pinnedAt?: [number, number] };
  size: [number, number];
}
const grid = (value: number): boolean => Number.isInteger(value) && value % 8 === 0;
const overlaps = (a: Region, b: Region, gap = 0): boolean =>
  a.x < b.x + b.width + gap && a.x + a.width + gap > b.x && a.y < b.y + b.height + gap && a.y + a.height + gap > b.y;
const contains = (area: Region, box: Region): boolean =>
  box.x >= area.x &&
  box.y >= area.y &&
  box.x + box.width <= area.x + area.width &&
  box.y + box.height <= area.y + area.height;

export function layoutOptions(layout: LayoutConstraints = {}): { gap: number; flow: "horizontal" | "vertical" } {
  if (
    !layout ||
    typeof layout !== "object" ||
    Array.isArray(layout) ||
    Object.keys(layout).some((key) => !["regions", "gap", "flow", "padding", "equalSize"].includes(key))
  )
    throw new Error("INVALID_ARGUMENT: unsupported layout constraints.");
  const gap = layout.gap ?? 24,
    flow = layout.flow ?? "horizontal";
  if (!grid(gap) || gap < 8 || gap > 128)
    throw new Error("INVALID_ARGUMENT: layout gap must be 8–128 pixels on the 8-pixel grid.");
  if (flow !== "horizontal" && flow !== "vertical")
    throw new Error("INVALID_ARGUMENT: layout flow must be horizontal or vertical.");
  if (layout.padding !== undefined && (!grid(layout.padding) || layout.padding < 0 || layout.padding > 128))
    throw new Error("INVALID_ARGUMENT: padding must use the 8-pixel grid between 0 and 128.");
  if (layout.equalSize !== undefined && !["width", "height", "both"].includes(layout.equalSize))
    throw new Error("INVALID_ARGUMENT: equalSize must be width, height or both.");
  return { gap, flow };
}

/** Resolve explicit regions against the same defaults used by ordinary composition. */
export function constrainedRegions(
  defaults: Map<string, Region>,
  bounds: Region,
  layout?: LayoutConstraints,
): Map<string, Region> {
  const areas = new Map(defaults);
  if (layout?.regions !== undefined) {
    if (!layout.regions || typeof layout.regions !== "object" || Array.isArray(layout.regions))
      throw new Error("INVALID_ARGUMENT: layout regions must be an object.");
    for (const [role, area] of Object.entries(layout.regions)) {
      if (!["process", "measurements", "trends", "alarms", "controls"].includes(role))
        throw new Error(`INVALID_ARGUMENT: unknown layout region ${role}.`);
      if (
        !area ||
        Object.keys(area).sort().join(",") !== "height,width,x,y" ||
        ![area.x, area.y, area.width, area.height].every(grid) ||
        area.width <= 0 ||
        area.height <= 0
      )
        throw new Error(`INVALID_ARGUMENT: ${role} region must use positive dimensions on the 8-pixel grid.`);
      if (!contains(bounds, area))
        throw new Error(`VALIDATION_FAILED: ${role} region does not fit below the heading and inside page margins.`);
      areas.set(role, area);
    }
  }
  const entries = [...areas];
  for (let a = 0; a < entries.length; a++)
    for (let b = a + 1; b < entries.length; b++) {
      if (overlaps(entries[a][1], entries[b][1]))
        throw new Error(
          `VALIDATION_FAILED: ${entries[a][0]} and ${entries[b][0]} regions overlap; move or resize the regions.`,
        );
    }
  return areas;
}

/** Reserve pinned geometry before placing movable items; never move or shrink a fixed item. */
export function placeRegion(
  items: SizedItem[],
  area: Region,
  role: string,
  gap: number,
  flow: "horizontal" | "vertical",
  obstacles: Array<{ id: string; box: Region }> = [],
): Map<string, [number, number]> {
  const placed = new Map<string, [number, number]>();
  const occupied: Array<{ id: string; box: Region }> = [...obstacles];
  const boxAt = (entry: SizedItem, at: [number, number]): Region =>
    drawnBox({ ...entry.item, at, size: entry.size, rotation: entry.item.rotation ?? 0 } as HmiObjectModel);
  for (const entry of items.filter((entry) => entry.item.pinnedAt !== undefined)) {
    const at = entry.item.pinnedAt!;
    if (!Array.isArray(at) || at.length !== 2 || !at.every(grid))
      throw new Error(`INVALID_ARGUMENT: ${entry.item.id} pinned position must use the 8-pixel grid.`);
    const box = boxAt(entry, at);
    if (!contains(area, box))
      throw new Error(`VALIDATION_FAILED: ${entry.item.id} does not fit in the ${role} region.`);
    const collision = occupied.find((other) => overlaps(box, other.box));
    if (collision) throw new Error(`VALIDATION_FAILED: pinned objects ${entry.item.id} and ${collision.id} overlap.`);
    placed.set(entry.item.id, [...at]);
    occupied.push({ id: entry.item.id, box });
  }
  const movable = items.filter((entry) => entry.item.pinnedAt === undefined);
  if (!movable.length) return placed;
  const boxes = movable.map((entry) => boxAt(entry, [0, 0]));
  const cellWidth = Math.max(...boxes.map((box) => box.width)),
    cellHeight = Math.max(...boxes.map((box) => box.height));
  const columns =
    role === "measurements" || role === "controls" ? 1 : Math.floor((area.width + gap) / (cellWidth + gap));
  const rows = Math.floor((area.height + gap) / (cellHeight + gap));
  for (const entry of movable) {
    const origin = boxAt(entry, [0, 0]);
    let found = false;
    for (let index = 0; index < rows * columns; index++) {
      const column = flow === "horizontal" ? index % columns : Math.floor(index / rows);
      const row = flow === "horizontal" ? Math.floor(index / columns) : index % rows;
      const at: [number, number] = [
        Math.floor((area.x + column * (cellWidth + gap) - origin.x) / 8) * 8,
        Math.floor(
          (area.y + row * (cellHeight + gap) + (role === "process" ? (cellHeight - origin.height) / 2 : 0) - origin.y) /
            8,
        ) * 8,
      ];
      const box = boxAt(entry, at);
      if (!contains(area, box) || occupied.some((other) => overlaps(box, other.box, gap))) continue;
      placed.set(entry.item.id, at);
      occupied.push({ id: entry.item.id, box });
      found = true;
      break;
    }
    if (!found)
      throw new Error(
        `VALIDATION_FAILED: ${entry.item.id} does not fit in ${role} at readable sizes; enlarge its region, reduce items or split the page.`,
      );
  }
  return placed;
}
