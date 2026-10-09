import { adaptPage } from "../drawing/adaptive";
import { drawnBox } from "../editor/editing";
import type { Catalogue } from "../editor/catalogue";
import type { HmiObjectModel, HmiPageModel } from "../drawing/model";

export interface ViewportProfile {
  id: string;
  width: number;
  height: number;
}
interface Rectangle {
  x: number;
  y: number;
  width: number;
  height: number;
}
const FINDING_LIMIT = 200;
const contains = (a: Rectangle, b: Rectangle): boolean =>
  b.x >= a.x && b.y >= a.y && b.x + b.width <= a.x + a.width && b.y + b.height <= a.y + a.height;

export function viewportProfiles(value: unknown): ViewportProfile[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > 3)
    throw new Error("INVALID_ARGUMENT: provide at most three target content viewports.");
  const ids = new Set<string>();
  for (const profile of value) {
    if (
      !profile ||
      typeof profile !== "object" ||
      Object.keys(profile).sort().join(",") !== "height,id,width" ||
      typeof profile.id !== "string" ||
      !/^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(profile.id) ||
      ids.has(profile.id) ||
      ![profile.width, profile.height].every((size) => Number.isInteger(size) && size > 0 && size <= 4096)
    )
      throw new Error("INVALID_ARGUMENT: viewports need unique IDs and positive dimensions up to 4096 pixels.");
    ids.add(profile.id);
  }
  return value;
}

/** Geometry is a review aid, not a beauty score or proof of a rendered physical panel. */
export function geometryQuality(
  page: HmiPageModel,
  size: [number, number],
  catalogue: Catalogue,
  profiles: ViewportProfile[],
) {
  const findings: Array<Record<string, unknown>> = [];
  let omitted = 0;
  const add = (finding: Record<string, unknown>): void => {
    if (findings.length < FINDING_LIMIT) findings.push({ severity: "warning", ...finding });
    else omitted++;
  };
  const objects = page.object.map((object) => ({ object, box: drawnBox(object) }));
  const pageBox = { x: 0, y: 0, width: size[0], height: size[1] };
  for (const { object, box } of objects)
    if (!contains(pageBox, box))
      add({
        rule: "outside-page",
        object: object.id,
        objects: [object.id],
        rectangle: box,
        measured: box,
        required: pageBox,
        message: `${object.id} extends beyond the design page; move or resize it within the page bounds.`,
      });
  const background = (object: HmiObjectModel): boolean => object.type === "group" || object.type === "panel";
  for (let first = 0; first < objects.length; first++)
    for (let second = first + 1; second < objects.length; second++) {
      const a = objects[first],
        b = objects[second];
      if (
        (background(a.object) && contains(a.box, b.box)) ||
        (background(b.object) && contains(b.box, a.box)) ||
        a.object.attach === b.object.id ||
        b.object.attach === a.object.id
      )
        continue;
      const x = Math.max(a.box.x, b.box.x),
        y = Math.max(a.box.y, b.box.y);
      const width = Math.min(a.box.x + a.box.width, b.box.x + b.box.width) - x,
        height = Math.min(a.box.y + a.box.height, b.box.y + b.box.height) - y;
      if (width > 0 && height > 0)
        add({
          rule: "object-overlap",
          objects: [a.object.id, b.object.id],
          rectangle: { x, y, width, height },
          message: `${a.object.id} and ${b.object.id} overlap; separate them or review the intended overlay.`,
        });
    }
  const viewports = profiles.map((profile) => {
    const adapted = adaptPage(page, size, [profile.width, profile.height]);
    const scale = adapted.fit ? Math.min(profile.width / adapted.size[0], profile.height / adapted.size[1]) : 1;
    const offsetX = adapted.fit ? (profile.width - adapted.size[0] * scale) / 2 : 0;
    const offsetY = adapted.fit ? (profile.height - adapted.size[1] * scale) / 2 : 0;
    const projected = adapted.page.object.map((object) => {
      const box = drawnBox(object);
      const rectangle = {
        x: offsetX + box.x * scale,
        y: offsetY + box.y * scale,
        width: box.width * scale,
        height: box.height * scale,
      };
      const touch =
        catalogue.components.find((component) => component.type === object.type)?.category === "control" ||
        object.type === "page-button" ||
        object.click !== undefined;
      if (touch && (rectangle.width < 44 || rectangle.height < 44))
        add({
          rule: "viewport-touch-target",
          object: object.id,
          objects: [object.id],
          viewport: profile.id,
          rectangle,
          measured: { width: rectangle.width, height: rectangle.height },
          required: { width: 44, height: 44 },
          message: `${object.id} is ${rectangle.width.toFixed(1)} × ${rectangle.height.toFixed(1)} px in ${profile.id}; use a larger control, readable scrolling or another page to retain a 44 × 44 px target.`,
        });
      return { id: object.id, ...rectangle };
    });
    return {
      ...profile,
      scale,
      origin: "shared-adaptive-layout",
      contentSize: adapted.size,
      scroll: !adapted.fit,
      objects: projected,
    };
  });
  return { findings, viewports, truncated: omitted > 0, omitted };
}
