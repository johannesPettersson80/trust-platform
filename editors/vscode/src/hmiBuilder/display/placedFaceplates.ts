// Faceplates placed on the page (specification 39, §8.9): each has its own command surface, named
// by its placement as its popup's is, sharing the session and the overlay; its values come with
// the page's stream. A click or press inside a placement goes to its surface, at the point in the
// faceplate's own design coordinates.

import type { HmiPageModel, LiveValue, PlacedFaceplate } from "../drawing/model";
import { PLACED_HEADER } from "../drawing/placed";
import type { CommandSurface } from "./commands";
import type { Operation } from "./operate";
import type { PlacedFaceplateData } from "./runtimeDisplay";

type Point = { x: number; y: number };

interface Placement {
  data: PlacedFaceplateData;
  commands: CommandSurface;
}

export class PlacedFaceplates {
  private readonly placements = new Map<string, Placement>();
  /** The placements whose faceplate the project lacks. */
  private readonly lacking: string[] = [];
  /** The placement a slider is being dragged in, from its press until the release. */
  private dragging: string | undefined;

  constructor(
    placed: Record<string, PlacedFaceplateData | null>,
    operation: Operation,
    redraw: () => void,
    liveAt: (path: string) => LiveValue | undefined,
    /** The page as laid out, where each placement stands. */
    private readonly page: () => HmiPageModel,
  ) {
    for (const [id, data] of Object.entries(placed)) {
      if (data)
        this.placements.set(id, {
          data,
          commands: operation.popupSurface({ name: data.name, model: data.model, roles: data.roles }, redraw, liveAt),
        });
      else this.lacking.push(id);
    }
  }

  /** What the drawing needs of each placement. */
  frame(): Record<string, PlacedFaceplate | null> {
    const frame: Record<string, PlacedFaceplate | null> = Object.fromEntries(this.lacking.map((id) => [id, null]));
    for (const [id, { data, commands }] of this.placements) {
      frame[id] = {
        model: data.model,
        commands: commands.states(),
        drafts: commands.drafts(),
        locked: commands.locked(),
      };
    }
    return frame;
  }

  /** A click inside a placement, on one of its objects; whether it was inside one. */
  click(target: Element | null, option: string | undefined): boolean {
    const found = this.within(target);
    if (!found) return false;
    if (found.object) found.placement.commands.click(found.object, option);
    return true;
  }

  /** A press inside a placement, at a point on the page; whether one of its controls took it. */
  press(target: Element | null, at: Point): boolean {
    const found = this.within(target);
    if (!found?.object) return false;
    const taken = found.placement.commands.press(found.object, this.inner(found.id, at));
    this.dragging = taken ? found.id : undefined;
    return taken;
  }

  drag(at: Point): void {
    if (this.dragging) this.placements.get(this.dragging)?.commands.drag(this.inner(this.dragging, at));
  }

  release(at: Point): void {
    const id = this.dragging;
    this.dragging = undefined;
    if (id) this.placements.get(id)?.commands.release(this.inner(id, at));
  }

  /** The placement a target lies in, and the object of its faceplate the target belongs to. */
  private within(target: Element | null): { id: string; placement: Placement; object?: string } | undefined {
    const box = target?.closest?.("[data-placed]");
    const id = box?.getAttribute("data-placed");
    const placement = id ? this.placements.get(id) : undefined;
    if (!box || !id || !placement) return undefined;
    const object = target?.closest?.("[data-object]");
    return {
      id,
      placement,
      object: object && box.contains(object) ? (object.getAttribute("data-object") ?? undefined) : undefined,
    };
  }

  /** A point on the page in a placement's own coordinates: the card keeps the faceplate's
   * proportions, centred in its object, with the header above the faceplate's objects. */
  private inner(id: string, at: Point): Point {
    const object = this.page().object.find((candidate) => candidate.id === id);
    const model = this.placements.get(id)?.data.model;
    if (!object || !model) return at;
    const [width, height] = object.size;
    const [faceplateWidth, faceplateHeight] = model.size;
    const total = faceplateHeight + PLACED_HEADER;
    const scale = Math.min(width / faceplateWidth, height / total);
    const left = object.at[0] + (width - faceplateWidth * scale) / 2;
    const top = object.at[1] + (height - total * scale) / 2;
    return { x: (at.x - left) / scale, y: (at.y - top) / scale - PLACED_HEADER };
  }
}
