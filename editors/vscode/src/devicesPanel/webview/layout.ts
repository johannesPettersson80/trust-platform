// Geometry for the canvas: where each item is drawn and how a link is routed (design §4.5 D1). It
// reads only identities and grouping from the view model — never a state, an enabled flag, a look
// or a rule: Rust decides what is shown and how it is drawn, and this module only places it
// (design §3.4). All sizes are constants, so a route is deterministic without measuring the DOM.

/** A rectangle on the canvas. */
export interface Rect {
  /** Its left edge. */
  x: number;
  /** Its top edge. */
  y: number;
  /** Its width. */
  width: number;
  /** Its height. */
  height: number;
}

/** A point on the canvas. */
export interface Point {
  /** Its horizontal position. */
  x: number;
  /** Its vertical position. */
  y: number;
}

/** A routed line: the polyline it is drawn as. */
export interface Route {
  /** The link's or drop's identity, to find its view-model fields again. */
  id: string;
  /** The corners of the route. */
  points: Point[];
}

/** A runtime and the connections it holds. */
export interface LayoutRuntime {
  /** The runtime's identity. */
  id: string;
  /** How many Edit slots it draws. */
  slots: number;
  /** The identities of its connections, in their order. */
  connections: string[];
}

/** A computer frame and the runtimes it holds. */
export interface LayoutFrame {
  /** The frame's identity. */
  id: string;
  /** How many Edit slots it draws in its header. */
  slots: number;
  /** Its runtimes, in their order. */
  runtimes: LayoutRuntime[];
}

/** A mesh bus and the items it drops to. */
export interface LayoutMesh {
  /** The bus's identity. */
  id: string;
  /** The identities of the items it drops to. */
  drops: string[];
}

/** A link between two items. */
export interface LayoutLink {
  /** The link's identity. */
  id: string;
  /** One end. */
  from: string;
  /** The other end. */
  to: string;
}

/** Everything `layoutCanvas` needs, and nothing it must not read. */
export interface LayoutScene {
  /** The frames, in their order. */
  frames: LayoutFrame[];
  /** The devices' identities, in their order. */
  devices: string[];
  /** The mesh buses, in their order. */
  mesh: LayoutMesh[];
  /** The links. */
  links: LayoutLink[];
}

/** Where everything is drawn, and the size of the canvas. */
export interface CanvasLayout {
  /** Frame rectangles, in canvas coordinates. */
  frames: Record<string, Rect>;
  /** Runtime rectangles, relative to their frame. */
  runtimes: Record<string, Rect>;
  /** Connection rectangles, relative to their runtime. */
  connections: Record<string, Rect>;
  /** Device rectangles, in canvas coordinates. */
  devices: Record<string, Rect>;
  /** Mesh bus rectangles, in canvas coordinates. */
  mesh: Record<string, Rect>;
  /** Link routes, in canvas coordinates. */
  links: Route[];
  /** Mesh drop routes, in canvas coordinates. */
  drops: Route[];
  /** The canvas width. */
  width: number;
  /** The canvas height. */
  height: number;
}

const MARGIN = 24;
const FRAME_PAD = 14;
const FRAME_HEADER = 44;
const RUNTIME_WIDTH = 224;
const RUNTIME_HEADER = 72;
const RUNTIME_GAP = 10;
const RUNTIME_ROW_GAP = 16;
const SLOT_STEP = 30;
const PILL_HEIGHT = 30;
const PILL_WIDTH = 196;
const PILL_INSET = 14;
const DEVICE_WIDTH = 220;
const DEVICE_HEIGHT = 56;
const DEVICE_GAP = 12;
const MESH_WIDTH = 220;
const MESH_HEIGHT = 44;
const COLUMN_GAP = 120;

/** The middle of a rectangle's right edge, where a link leaves it. */
function rightCenter(rect: Rect): Point {
  return { x: rect.x + rect.width, y: rect.y + rect.height / 2 };
}

/** The middle of a rectangle's left edge, where a link reaches it. */
function leftCenter(rect: Rect): Point {
  return { x: rect.x, y: rect.y + rect.height / 2 };
}

/** An orthogonal route from one point to another, with a single vertical lane between them. */
function route(id: string, from: Point, to: Point): Route {
  const lane = (from.x + to.x) / 2;
  return { id, points: [from, { x: lane, y: from.y }, { x: lane, y: to.y }, to] };
}

/** Routes a link between two items, leaving the left one on its right edge. */
function routeBetween(id: string, a: Rect, b: Rect): Route {
  const [left, right] = a.x <= b.x ? [a, b] : [b, a];
  return route(id, rightCenter(left), leftCenter(right));
}

/** The identity of a mesh drop's route, so the bus and the drop can be found again. */
export function dropRouteId(bus: string, item: string): string {
  return `${bus}:${item}`;
}

/** Places every frame, runtime, connection, device and mesh bus, and routes every link and drop. */
export function layoutCanvas(scene: LayoutScene): CanvasLayout {
  const frames: Record<string, Rect> = {};
  const runtimes: Record<string, Rect> = {};
  const connections: Record<string, Rect> = {};
  const origins: Record<string, Rect | undefined> = {};

  let framesColumnWidth = 0;
  let frameY = MARGIN;
  for (const frame of scene.frames) {
    const count = frame.runtimes.length;
    const frameWidth = FRAME_PAD * 2 + count * RUNTIME_WIDTH + Math.max(0, count - 1) * RUNTIME_GAP;
    const headerHeight = FRAME_HEADER + frame.slots * SLOT_STEP;
    let runtimeHeight = RUNTIME_HEADER;
    for (const runtime of frame.runtimes) {
      runtimeHeight = Math.max(
        runtimeHeight,
        RUNTIME_HEADER +
          runtime.slots * SLOT_STEP +
          runtime.connections.length * PILL_HEIGHT +
          RUNTIME_ROW_GAP,
      );
    }
    const frameRect: Rect = {
      x: MARGIN,
      y: frameY,
      width: frameWidth,
      height: headerHeight + FRAME_PAD * 2 + runtimeHeight,
    };
    frames[frame.id] = frameRect;
    origins[frame.id] = frameRect;
    frame.runtimes.forEach((runtime, index) => {
      const local: Rect = {
        x: FRAME_PAD + index * (RUNTIME_WIDTH + RUNTIME_GAP),
        y: headerHeight + FRAME_PAD,
        width: RUNTIME_WIDTH,
        height: runtimeHeight,
      };
      runtimes[runtime.id] = local;
      const absolute: Rect = {
        x: frameRect.x + local.x,
        y: frameRect.y + local.y,
        width: local.width,
        height: local.height,
      };
      origins[runtime.id] = absolute;
      runtime.connections.forEach((connection, pillIndex) => {
        const pill: Rect = {
          x: PILL_INSET,
          y: RUNTIME_HEADER + runtime.slots * SLOT_STEP + pillIndex * PILL_HEIGHT,
          width: PILL_WIDTH,
          height: PILL_HEIGHT - 6,
        };
        connections[connection] = pill;
        origins[connection] = {
          x: absolute.x + pill.x,
          y: absolute.y + pill.y,
          width: pill.width,
          height: pill.height,
        };
      });
    });
    framesColumnWidth = Math.max(framesColumnWidth, frameWidth);
    frameY += frameRect.height + RUNTIME_ROW_GAP;
  }

  const devicesX = MARGIN + framesColumnWidth + COLUMN_GAP;
  const devices: Record<string, Rect> = {};
  let deviceY = MARGIN;
  for (const device of scene.devices) {
    const rect: Rect = { x: devicesX, y: deviceY, width: DEVICE_WIDTH, height: DEVICE_HEIGHT };
    devices[device] = rect;
    origins[device] = rect;
    deviceY += DEVICE_HEIGHT + DEVICE_GAP;
  }

  const meshX = devicesX + DEVICE_WIDTH + COLUMN_GAP;
  const mesh: Record<string, Rect> = {};
  let meshY = MARGIN;
  for (const bus of scene.mesh) {
    const rect: Rect = { x: meshX, y: meshY, width: MESH_WIDTH, height: MESH_HEIGHT };
    mesh[bus.id] = rect;
    origins[bus.id] = rect;
    meshY += MESH_HEIGHT + DEVICE_GAP;
  }

  const links: Route[] = [];
  for (const link of scene.links) {
    const from = origins[link.from];
    const to = origins[link.to];
    if (from !== undefined && to !== undefined) {
      links.push(routeBetween(link.id, from, to));
    }
  }

  const drops: Route[] = [];
  for (const bus of scene.mesh) {
    const busRect = mesh[bus.id];
    for (const item of bus.drops) {
      const target = origins[item];
      if (busRect !== undefined && target !== undefined) {
        drops.push(route(dropRouteId(bus.id, item), leftCenter(busRect), rightCenter(target)));
      }
    }
  }

  return {
    frames,
    runtimes,
    connections,
    devices,
    mesh,
    links,
    drops,
    width: meshX + MESH_WIDTH + MARGIN,
    height: Math.max(frameY, deviceY, meshY, MARGIN) + MARGIN,
  };
}
