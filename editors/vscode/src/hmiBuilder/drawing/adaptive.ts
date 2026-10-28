// Adaptive display (specification 39, §4 item 5): a page laid out for a page area smaller than
// its design size, at scale 1, so its texts and touch targets keep their sizes. The page is
// divided into blocks, each top-level panel or group with what lies inside it and the rest of
// the page as the process graphic; blocks that do not fit move, and a block too wide scales or,
// arranged, rearranges its objects.

import type { HmiObjectModel, HmiPageModel } from "./model";
import { symbolFor } from "./symbols";

/** The page as the area shows it: its objects and pipes, its size, and whether it is scaled to
 * fit the area (an area at least the design size) or shown at scale 1 and scrolled. */
export interface AdaptedPage {
  page: HmiPageModel;
  size: [number, number];
  fit: boolean;
}

/** The space between blocks, and an arranged container's padding and default gap. */
const SPACE = 16;
/** The room a titled panel or group keeps above its objects for its title. */
const TITLE = 44;
/** The least width and height of a touch target (§3.5 item 2). */
const TOUCH = 44;
const CONTAINERS = new Set(["panel", "group"]);
/** The components an operator touches, which keep their touch size. */
const CONTROLS = new Set([
  "button",
  "pulse-button",
  "switch",
  "selector",
  "mode-selector",
  "slider",
  "numeric-input",
  "setpoint",
  "page-button",
]);

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** An object with what lies inside it, which moves and scales with it. */
interface Item {
  object: HmiObjectModel;
  inside: Item[];
}

/** A block: a top-level container with what lies inside it, a display, control or text on its
 * own, or the process graphic. */
interface Block {
  container?: Item;
  items: Item[];
  box: Box;
  /** The process graphic, whose pipes' bends move with it. */
  graphic?: boolean;
}

/** Equipment, junctions, lines and images draw the process graphic together. */
function isGraphic(type: string): boolean {
  return symbolFor(type) !== undefined || type === "junction" || type === "line" || type === "image";
}

/** A block laid out: its size, and where each of its objects goes relative to its corner. */
interface Laid {
  width: number;
  height: number;
  objects: Map<string, { at: [number, number]; size: [number, number] }>;
  /** Where a point of the design goes, relative to the block's corner. */
  point: (point: [number, number]) => [number, number];
}

/** The page laid out for a page area of `area` pixels; `design` is its design size. */
export function adaptPage(page: HmiPageModel, design: [number, number], area: [number, number]): AdaptedPage {
  // Shared regions reserve real screen space. Keep their authored layout and scroll when
  // needed; automatic block reflow cannot move a page object into its shared sidebar.
  if (page.regions?.length) return { page, size: design, fit: area[0] >= design[0] && area[1] >= design[1] };
  if (area[0] >= design[0] && area[1] >= design[1]) return { page, size: design, fit: true };
  if (area[0] >= design[0]) return { page, size: design, fit: false };
  const width = area[0];
  const blocks = blocksOf(page);
  if (blocks.length === 0) return { page, size: [width, area[1]], fit: false };
  const margin = Math.max(0, Math.min(...blocks.map((block) => block.box.x)));
  const top = Math.max(0, Math.min(...blocks.map((block) => block.box.y)));
  const placed: Box[] = [];
  const objects = new Map<string, { at: [number, number]; size: [number, number] }>();
  let pipePoint: ((point: [number, number]) => [number, number]) | undefined;
  for (const block of readingOrder(blocks)) {
    const laid = layOut(block, width - 2 * margin);
    let x = block.box.x;
    if (x + laid.width > width - margin) x = Math.max(margin, width - margin - laid.width);
    const box: Box = { x, y: block.box.y, width: laid.width, height: laid.height };
    // A block that would overlap where it fits starts a new row at the left margin.
    if (placed.some((other) => overlap(box, other))) box.x = margin;
    for (
      let blocking = placed.filter((other) => overlap(box, other));
      blocking.length > 0;
      blocking = placed.filter((other) => overlap(box, other))
    ) {
      box.y = Math.max(...blocking.map((other) => other.y + other.height)) + SPACE;
    }
    placed.push(box);
    for (const [id, where] of laid.objects)
      objects.set(id, { at: [box.x + where.at[0], box.y + where.at[1]], size: where.size });
    if (block.graphic) pipePoint = (point) => add(laid.point(point), [box.x, box.y]);
  }
  const right = Math.max(...placed.map((box) => box.x + box.width));
  const bottom = Math.max(...placed.map((box) => box.y + box.height));
  return {
    page: {
      ...page,
      object: page.object.map((object) => {
        const where = objects.get(object.id);
        return where ? { ...object, at: where.at, size: where.size } : object;
      }),
      pipe: page.pipe.map((pipe) =>
        pipe.waypoints && pipePoint ? { ...pipe, waypoints: pipe.waypoints.map(pipePoint) } : pipe,
      ),
    },
    size: [Math.max(width, right + margin), Math.max(area[1], bottom + top)],
    fit: false,
  };
}

/** The blocks in reading order (§4 item 5): by bands top to bottom, a band being the blocks whose
 * heights overlap, directly or through others; within a band by columns left to right, a column
 * being the blocks whose widths overlap; within a column top to bottom. */
function readingOrder(blocks: Block[]): Block[] {
  const bands: Block[][] = [];
  let bandBottom = -Infinity;
  for (const block of [...blocks].sort((a, b) => a.box.y - b.box.y || a.box.x - b.box.x)) {
    const bottom = block.box.y + block.box.height;
    if (bands.length > 0 && block.box.y < bandBottom) {
      bands[bands.length - 1].push(block);
      bandBottom = Math.max(bandBottom, bottom);
    } else {
      bands.push([block]);
      bandBottom = bottom;
    }
  }
  return bands.flatMap((band) => {
    const columns: Array<{ right: number; blocks: Block[] }> = [];
    for (const block of [...band].sort((a, b) => a.box.x - b.box.x || a.box.y - b.box.y)) {
      const last = columns[columns.length - 1];
      if (last && block.box.x < last.right) {
        last.blocks.push(block);
        last.right = Math.max(last.right, block.box.x + block.box.width);
      } else {
        columns.push({ right: block.box.x + block.box.width, blocks: [block] });
      }
    }
    return columns.flatMap((column) => column.blocks.sort((a, b) => a.box.y - b.box.y || a.box.x - b.box.x));
  });
}

/** The page's blocks: each top-level container with what lies wholly inside it, each other
 * display, control and text on its own, and the equipment, lines and images with the pipes' bends
 * as the process graphic. */
function blocksOf(page: HmiPageModel): Block[] {
  const containers = page.object.filter((object) => CONTAINERS.has(object.type));
  // Each object lies in the smallest container that holds it wholly.
  const holder = new Map<string, string>();
  for (const object of page.object) {
    const holders = containers.filter((container) => container.id !== object.id && holds(container, object));
    holders.sort((a, b) => a.size[0] * a.size[1] - b.size[0] * b.size[1]);
    if (holders[0]) holder.set(object.id, holders[0].id);
  }
  const itemOf = (object: HmiObjectModel): Item => ({
    object,
    inside: page.object.filter((other) => holder.get(other.id) === object.id).map(itemOf),
  });
  const loose = page.object.filter((object) => !holder.has(object.id));
  const blocks: Block[] = loose
    .filter((object) => CONTAINERS.has(object.type))
    .map((object) => ({ container: itemOf(object), items: [], box: boxOf(object) }));
  const graphic = loose.filter((object) => !CONTAINERS.has(object.type) && isGraphic(object.type));
  for (const object of loose.filter((candidate) => !CONTAINERS.has(candidate.type) && !isGraphic(candidate.type))) {
    blocks.push({ items: [itemOf(object)], box: boxOf(object) });
  }
  if (graphic.length > 0) {
    const points = [
      ...graphic.flatMap((object) => [object.at, add(object.at, object.size)]),
      ...page.pipe.flatMap((pipe) => pipe.waypoints ?? []),
    ];
    const [left, upper] = [Math.min(...points.map((point) => point[0])), Math.min(...points.map((point) => point[1]))];
    const box = {
      x: left,
      y: upper,
      width: Math.max(...points.map((point) => point[0])) - left,
      height: Math.max(...points.map((point) => point[1])) - upper,
    };
    blocks.push({ items: graphic.map(itemOf), box, graphic: true });
  }
  return blocks;
}

/** A block at the width between the margins: as designed when it fits; else an arranged
 * container rearranges its objects and any other block scales uniformly. */
function layOut(block: Block, inner: number): Laid {
  const items = block.container ? [block.container] : block.items;
  const all = items.flatMap(flatten);
  if (block.box.width <= inner) return scaled(all, block.box, 1);
  const container = block.container?.object;
  if (block.container && container?.arrange) return arranged(block.container, inner);
  return scaled(all, block.box, scaleFor(all, inner / block.box.width));
}

/** The objects scaled by `k` about the box's corner, each to whole pixels. */
function scaled(objects: HmiObjectModel[], box: Box, k: number): Laid {
  const point = (point: [number, number]): [number, number] => [
    Math.round((point[0] - box.x) * k),
    Math.round((point[1] - box.y) * k),
  ];
  const laid = new Map(
    objects.map((object) => [
      object.id,
      {
        at: point(object.at),
        size: [Math.round(object.size[0] * k), Math.round(object.size[1] * k)] as [number, number],
      },
    ]),
  );
  return { width: Math.round(box.width * k), height: Math.round(box.height * k), objects: laid, point };
}

/** The scale wanted, but no smaller than keeps every control at its touch size; never larger
 * than 1. */
function scaleFor(objects: HmiObjectModel[], wanted: number): number {
  const least = Math.max(
    0,
    ...objects
      .filter((object) => CONTROLS.has(object.type))
      .map((object) => Math.min(1, Math.max(TOUCH / object.size[0], TOUCH / object.size[1]))),
  );
  return Math.min(1, Math.max(wanted, least));
}

/** An arranged container at the width between the margins, its objects in reading order inside
 * its padding (§4 item 5): in a row, a column or a grid; each keeps its size unless wider than
 * the room, and what lies inside one moves with it. */
function arranged(container: Item, width: number): Laid {
  const object = container.object;
  const gap = object.gap ?? SPACE;
  const upper = object.label ? TITLE : SPACE;
  const room = width - 2 * SPACE;
  const items = [...container.inside].sort(
    (a, b) => a.object.at[1] - b.object.at[1] || a.object.at[0] - b.object.at[0],
  );
  // Each item laid out on its own, shrunk to the room when wider.
  const pieces = items.map((item) => {
    const box = boxOf(item.object);
    const all = flatten(item);
    return scaled(all, box, box.width > room ? scaleFor(all, room / box.width) : 1);
  });
  const corners: Array<[number, number]> = [];
  let height = 0;
  if (object.arrange === "grid") {
    const cell: [number, number] = [
      Math.max(0, ...pieces.map((piece) => piece.width)),
      Math.max(0, ...pieces.map((piece) => piece.height)),
    ];
    const columns = Math.max(1, Math.floor((room + gap) / (cell[0] + gap)));
    pieces.forEach((_piece, index) =>
      corners.push([(index % columns) * (cell[0] + gap), Math.floor(index / columns) * (cell[1] + gap)]),
    );
    height = pieces.length > 0 ? Math.ceil(pieces.length / columns) * (cell[1] + gap) - gap : 0;
  } else if (object.arrange === "column") {
    for (const piece of pieces) {
      corners.push([0, height]);
      height += piece.height + gap;
    }
    height = Math.max(0, height - gap);
  } else {
    let [x, y, line] = [0, 0, 0];
    for (const piece of pieces) {
      if (x > 0 && x + piece.width > room) [x, y, line] = [0, y + line + gap, 0];
      corners.push([x, y]);
      x += piece.width + gap;
      line = Math.max(line, piece.height);
    }
    height = y + line;
  }
  const objects = new Map<string, { at: [number, number]; size: [number, number] }>();
  const total = upper + height + SPACE;
  objects.set(object.id, { at: [0, 0], size: [width, total] });
  pieces.forEach((piece, index) => {
    for (const [id, where] of piece.objects)
      objects.set(id, { at: add(where.at, [SPACE + corners[index][0], upper + corners[index][1]]), size: where.size });
  });
  const origin = boxOf(object);
  return { width, height: total, objects, point: (point) => [point[0] - origin.x, point[1] - origin.y] };
}

function flatten(item: Item): HmiObjectModel[] {
  return [item.object, ...item.inside.flatMap(flatten)];
}

function boxOf(object: HmiObjectModel): Box {
  return { x: object.at[0], y: object.at[1], width: object.size[0], height: object.size[1] };
}

function holds(container: HmiObjectModel, object: HmiObjectModel): boolean {
  const [outer, inner] = [boxOf(container), boxOf(object)];
  return (
    inner.x >= outer.x &&
    inner.y >= outer.y &&
    inner.x + inner.width <= outer.x + outer.width &&
    inner.y + inner.height <= outer.y + outer.height &&
    inner.width * inner.height < outer.width * outer.height
  );
}

function overlap(a: Box, b: Box): boolean {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

function add(a: [number, number], b: [number, number]): [number, number] {
  return [a[0] + b[0], a[1] + b[1]];
}
