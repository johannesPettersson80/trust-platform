import { drawnBox } from "../editor/editing";
import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import { constrainedRegions, layoutOptions, placeRegion, Region } from "./layout";

export function equalSize(
  page: HmiPageModel,
  objects: HmiObjectModel[],
  reference: HmiObjectModel,
  axis: unknown,
): HmiPageModel {
  if (!["width", "height", "both"].includes(String(axis)))
    throw new Error("INVALID_ARGUMENT: equal-size requires width, height or both.");
  const sizes = new Map<string, [number, number]>();
  for (const object of objects) {
    let width = axis === "height" ? object.size[0] : reference.size[0],
      height = axis === "width" ? object.size[1] : reference.size[1];
    if (object.type === "faceplate") {
      const ratio = object.size[1] / object.size[0];
      if (axis === "both" && Math.abs(height - Math.round(width * ratio)) > 1)
        throw new Error(
          `VALIDATION_FAILED: ${object.id} cannot match both dimensions without changing faceplate proportions.`,
        );
      if (axis === "width") height = Math.round(width * ratio);
      if (axis === "height") width = Math.round(height / ratio);
    }
    if (width < 8 || height < 8)
      throw new Error(`VALIDATION_FAILED: ${object.id} would become smaller than the minimum dimensions.`);
    sizes.set(object.id, [width, height]);
  }
  return {
    ...page,
    object: page.object.map((object) => (sizes.has(object.id) ? { ...object, size: sizes.get(object.id)! } : object)),
  };
}

export function arrangeRegion(
  page: HmiPageModel,
  objects: HmiObjectModel[],
  operation: Record<string, unknown>,
  size?: [number, number],
): HmiPageModel {
  if (!size) throw new Error("INVALID_ARGUMENT: regional arrangement needs the project's design size.");
  const { gap, flow } = layoutOptions({
    gap: operation.gap as number | undefined,
    flow: operation.flow as "horizontal" | "vertical" | undefined,
  });
  const areas = constrainedRegions(
    new Map(),
    { x: 0, y: 0, width: size[0], height: size[1] },
    { regions: { process: operation.region as Region } },
  );
  const region = areas.get("process")!;
  const ids = new Set(objects.map((object) => object.id));
  const pinned = operation.pinnedIds ?? [];
  if (
    !Array.isArray(pinned) ||
    pinned.some((id) => typeof id !== "string" || !ids.has(id)) ||
    new Set(pinned).size !== pinned.length
  )
    throw new Error("INVALID_ARGUMENT: pinned IDs must be unique members of the arranged selection.");
  const obstacles = page.object
    .filter((object) => !ids.has(object.id) && !["group", "panel"].includes(object.type))
    .map((object) => ({ id: object.id, box: drawnBox(object) }));
  const entries = objects.map((object) => ({
    item: { ...object, ...(object.locked || pinned.includes(object.id) ? { pinnedAt: object.at } : {}) },
    size: object.size,
  }));
  const placed = placeRegion(entries, region, "process", gap, flow, obstacles);
  return {
    ...page,
    object: page.object.map((object) => (placed.has(object.id) ? { ...object, at: placed.get(object.id)! } : object)),
  };
}
