// Editing a page in the HMI Builder (specification 39, §3.3): adding, selecting, moving, resizing
// and deleting objects on the 8 px grid. Pure functions on the page model, without a DOM, so the
// webview and the tests share them; every change returns a new page.

import type { HmiObjectModel, HmiPageModel } from "../drawing/model";
import { afterDeleting, movedAttachments } from "./attachments";

export const GRID = 8;

export type Point = [number, number];
export type Handle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
export type Direction = "left" | "right" | "up" | "down";

/** A coordinate on the grid, or in whole pixels when placed freely (Alt). */
export function snap(value: number, free = false): number {
  return free ? Math.round(value) : Math.round(value / GRID) * GRID;
}

/** The first free id of the form `<type>-<n>` (§5.1). */
export function nextId(page: HmiPageModel, type: string): string {
  const used = new Set(page.object.map((object) => object.id));
  for (let number = 1; ; number += 1) {
    const id = `${type}-${number}`;
    if (!used.has(id)) return id;
  }
}

function changeObjects(
  page: HmiPageModel,
  ids: string[],
  change: (object: HmiObjectModel) => HmiObjectModel,
): HmiPageModel {
  return { ...page, object: page.object.map((object) => (ids.includes(object.id) ? change(object) : object)) };
}

/** Adds an object of a component with its default size, its top-left corner at `at`. */
export function addObject(
  page: HmiPageModel,
  component: { type: string; size: [number, number] },
  at: Point,
  free = false,
): { page: HmiPageModel; id: string } {
  const id = nextId(page, component.type);
  const created: HmiObjectModel = {
    id,
    type: component.type,
    at: [snap(at[0], free), snap(at[1], free)],
    size: [component.size[0], component.size[1]],
    rotation: 0,
    bind: {},
  };
  return { page: { ...page, object: [...page.object, created] }, id };
}

/** The top-left corner that centres an object of `size` on `centre`, kept on the page. */
export function cornerFor(size: [number, number], centre: Point, page: [number, number]): Point {
  return [
    Math.max(0, Math.min(page[0] - size[0], centre[0] - size[0] / 2)),
    Math.max(0, Math.min(page[1] - size[1], centre[1] - size[1] / 2)),
  ];
}

/** A corner no object of the page has yet: each step moves 24 px down and right within the page,
 * so objects placed one after another stay visible instead of piling up on one spot. */
export function freeCorner(page: HmiPageModel, corner: Point, size: [number, number], screen: [number, number]): Point {
  const step = 24;
  let [x, y] = corner;
  for (let tries = 0; tries < 40; tries += 1) {
    const taken = page.object.some(
      (object) => Math.abs(object.at[0] - x) < step / 2 && Math.abs(object.at[1] - y) < step / 2,
    );
    if (!taken) break;
    x = Math.max(0, Math.min(screen[0] - size[0], x + step));
    y = Math.max(0, Math.min(screen[1] - size[1], y + step));
  }
  return [x, y];
}

/** Moves objects together: the first of `ids` (the one dragged) lands on the grid, and the others
 * keep their offsets from it. */
export function moveObjects(page: HmiPageModel, ids: string[], delta: Point, free = false): HmiPageModel {
  const anchor = page.object.find((object) => object.id === ids[0]);
  if (!anchor) return page;
  const shift: Point = [
    snap(anchor.at[0] + delta[0], free) - anchor.at[0],
    snap(anchor.at[1] + delta[1], free) - anchor.at[1],
  ];
  return changeObjects(page, ids, (object) => ({ ...object, at: [object.at[0] + shift[0], object.at[1] + shift[1]] }));
}

/** An arrow key: one grid step, or one pixel with Alt. */
export function nudge(page: HmiPageModel, ids: string[], direction: Direction, fine = false): HmiPageModel {
  const step = fine ? 1 : GRID;
  const [dx, dy] = { left: [-step, 0], right: [step, 0], up: [0, -step], down: [0, step] }[direction];
  return changeObjects(page, ids, (object) => ({ ...object, at: [object.at[0] + dx, object.at[1] + dy] }));
}

/** Shift+arrow (§3.5): right and down grow the objects by one grid step, left and up shrink
 * them, never below one step; with Alt by one pixel, never below one pixel. */
export function resizeBy(page: HmiPageModel, ids: string[], direction: Direction, fine = false): HmiPageModel {
  const step = fine ? 1 : GRID;
  const [dw, dh] = { left: [-step, 0], right: [step, 0], up: [0, -step], down: [0, step] }[direction];
  return changeObjects(page, ids, (object) => ({
    ...object,
    size: [Math.max(step, object.size[0] + dw), Math.max(step, object.size[1] + dh)],
  }));
}

/** Drags a resize handle: the edges it holds move onto the grid, never closer than one step to
 * the opposite edge. A placed faceplate keeps its proportions (§8.9 item 4): its width leads, or
 * its height when the handle holds neither side. */
export function resizeObject(page: HmiPageModel, id: string, handle: Handle, delta: Point, free = false): HmiPageModel {
  const target = page.object.find((object) => object.id === id);
  if (!target) return page;
  let [left, top] = target.at;
  let right = left + target.size[0];
  let bottom = top + target.size[1];
  if (handle.includes("w")) left = Math.min(snap(left + delta[0], free), right - GRID);
  if (handle.includes("e")) right = Math.max(snap(right + delta[0], free), left + GRID);
  if (handle.includes("n")) top = Math.min(snap(top + delta[1], free), bottom - GRID);
  if (handle.includes("s")) bottom = Math.max(snap(bottom + delta[1], free), top + GRID);
  if (target.type === "faceplate") {
    const ratio = target.size[1] / target.size[0];
    const sideways = handle.includes("e") || handle.includes("w");
    const width = sideways ? right - left : Math.round((bottom - top) / ratio);
    const height = sideways ? Math.round(width * ratio) : bottom - top;
    if (handle.includes("w")) left = right - width;
    else right = left + width;
    if (handle.includes("n")) top = bottom - height;
    else bottom = top + height;
  }
  return changeObjects(page, [id], (object) => ({ ...object, at: [left, top], size: [right - left, bottom - top] }));
}

/** Deletes objects, and the pipes that join them (§6); the attachments to either go. */
export function deleteObjects(page: HmiPageModel, ids: string[]): HmiPageModel {
  const joined = (end: string): boolean => ids.includes(end.slice(0, end.lastIndexOf(".")));
  const gone = new Set(page.pipe.flatMap((pipe, index) => (joined(pipe.from) || joined(pipe.to) ? [index] : [])));
  const kept = {
    ...page,
    object: page.object.filter((object) => !ids.includes(object.id)),
    pipe: page.pipe.filter((_, index) => !gone.has(index)),
  };
  return movedAttachments(kept, afterDeleting(gone), ids);
}

/** The box an object is drawn in: a quarter turn swaps its width and height about its centre. */
export function drawnBox(object: HmiObjectModel): { x: number; y: number; width: number; height: number } {
  const [x, y] = object.at;
  const [width, height] = object.size;
  if (object.rotation % 180 === 0) return { x, y, width, height };
  return { x: x + width / 2 - height / 2, y: y + height / 2 - width / 2, width: height, height: width };
}

/** The topmost object whose drawn box holds the point. */
export function objectAt(page: HmiPageModel, point: Point): string | undefined {
  for (let index = page.object.length - 1; index >= 0; index -= 1) {
    const box = drawnBox(page.object[index]);
    if (point[0] >= box.x && point[0] <= box.x + box.width && point[1] >= box.y && point[1] <= box.y + box.height) {
      return page.object[index].id;
    }
  }
  return undefined;
}

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** The design point at the centre of the part of the page that is visible in the canvas (§3.3);
 * the page's centre while nothing is laid out. */
export function visiblePageCentre(area: Rect, sheet: Rect, zoom: number, size: [number, number]): Point {
  const left = Math.max(area.left, sheet.left);
  const right = Math.min(area.left + area.width, sheet.left + sheet.width);
  const top = Math.max(area.top, sheet.top);
  const bottom = Math.min(area.top + area.height, sheet.top + sheet.height);
  if (area.width === 0 || sheet.width === 0 || right <= left || bottom <= top) return [size[0] / 2, size[1] / 2];
  return [((left + right) / 2 - sheet.left) / zoom, ((top + bottom) / 2 - sheet.top) / zoom];
}

/** The objects wholly inside the box between two points (§3.3 item 8). */
export function objectsInBox(page: HmiPageModel, from: Point, to: Point): string[] {
  const [left, right] = from[0] <= to[0] ? [from[0], to[0]] : [to[0], from[0]];
  const [top, bottom] = from[1] <= to[1] ? [from[1], to[1]] : [to[1], from[1]];
  return page.object
    .filter((object) => {
      const box = drawnBox(object);
      return box.x >= left && box.y >= top && box.x + box.width <= right && box.y + box.height <= bottom;
    })
    .map((object) => object.id);
}

/** A click on an object (or on empty canvas, `undefined`), with Shift to add or remove. */
export function selectFromClick(selection: string[], id: string | undefined, additive: boolean): string[] {
  if (id === undefined) return additive ? selection : [];
  if (!additive) return [id];
  return selection.includes(id) ? selection.filter((selected) => selected !== id) : [...selection, id];
}

/** An inspector edit: positions and sizes stay whole pixels, and sizes at least one step. */
export function updateObject(page: HmiPageModel, id: string, patch: Partial<HmiObjectModel>): HmiPageModel {
  return changeObjects(page, [id], (object) => {
    const next = { ...object, ...patch };
    return {
      ...next,
      at: [Math.round(next.at[0]), Math.round(next.at[1])],
      size: [Math.max(GRID, Math.round(next.size[0])), Math.max(GRID, Math.round(next.size[1]))],
    };
  });
}
