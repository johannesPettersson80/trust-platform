// Thermal equipment (specification 39, §5.2 phase 2, thermal; §5.3 items 4 and 5): the cold room
// with its door, the refrigeration unit and the cooler with their fans, the electric heater with
// its water vessel, the jacketed vessel, the coupled vessels with their heat-transfer wall and the
// heat draw-off. A heater's element and a draw-off's heat show while they work.

import { el, SvgNode } from "../svg";
import type { SymbolDefinition, SymbolState } from "./index";
import { percentWord, readNumbers, readRunning, runningStates, shownBoundNumber, SymbolReader } from "./reading";
import { BODY, HATCH, LINE, MOVING, round, STILL } from "./style";
import { RoundedRect, roundedLiquid, roundedRectOutline } from "./vessels";

/** The vessels' walls, in their symbols' own pixels; the outlines an attached instrument meets (§21.6). */
const HEATER_VESSEL: RoundedRect = { x: 8, y: 6, width: 48, height: 52, rx: 6 };
const JACKET = "M10 22 V50 Q10 60 20 60 H44 Q54 60 54 50 V22 Z";
const JACKETED: RoundedRect = { x: 16, y: 6, width: 32, height: 50, rx: 8 };
const COUPLED: RoundedRect = { x: 6, y: 6, width: 84, height: 52, rx: 6 };

/** A fan of three vanes about (`x`, `y`), turning while `spin` says so. */
function fan(x: number, y: number, radius: number, spin: { delayMs: number } | undefined): SvgNode {
  const paint = spin ? MOVING : STILL;
  const point = (r: number, degrees: number): string => {
    const angle = (degrees * Math.PI) / 180;
    return `${round(x + r * Math.cos(angle))} ${round(y + r * Math.sin(angle))}`;
  };
  const vanes = [0, 120, 240]
    .map((root) => `M${point(3, root)} Q${point(radius * 0.7, root - 10)} ${point(radius * 0.85, root - 50)}`)
    .join(" ");
  return el(
    "g",
    {
      "data-part": "fan",
      class: spin ? "hmi-spin" : undefined,
      style: spin ? `animation-delay: -${spin.delayMs}ms` : undefined,
    },
    [
      el("circle", { cx: x, cy: y, r: radius, fill: "none", stroke: "none" }),
      el("path", {
        d: vanes,
        fill: "none",
        stroke: paint,
        "stroke-width": 2.5,
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke",
      }),
      el("circle", { cx: x, cy: y, r: 2.5, fill: paint }),
    ],
  );
}

/** Heat rising: three small waves above `y`, starting at the given x positions. */
function heat(xs: number[], y: number): SvgNode {
  const waves = xs.map((x) => `M${x} ${y} q3 -3 0 -6 q-3 -3 0 -6`).join(" ");
  return el("path", {
    "data-part": "heat",
    d: waves,
    fill: "none",
    stroke: MOVING,
    "stroke-width": 1.5,
    "stroke-linecap": "round",
    "vector-effect": "non-scaling-stroke",
  });
}

/** The word of a vessel: its temperature, else its level in percent. */
const readTemperature: SymbolReader = (value, frame, faulted, object) => {
  const reading = readNumbers("temperature")(value, frame, faulted, object);
  return reading.word || faulted
    ? reading
    : { ...reading, word: percentWord(value("level")) ?? percentWord(value("power")) ?? "" };
};

const DOOR = { hinge: [76, 46] as const, length: 20, opens: 75 };

export const COLD_ROOM: SymbolDefinition = {
  size: [96, 64],
  ports: {},
  properties: ["door-open", "temperature"],
  deciding: ["temperature", "door-open"],
  states: [
    {
      id: "closed",
      name: "Door shut",
      values: { "door-open": false, temperature: { share: 0.25 } },
      needs: ["door-open"],
    },
    { id: "open", name: "Door open", values: { "door-open": true, temperature: { share: 0.4 } }, needs: ["door-open"] },
  ],
  sample: "closed",
  read: (value, frame, faulted, object) => {
    const reading = readNumbers("temperature")(value, frame, faulted, object);
    const open = value("door-open");
    return {
      ...reading,
      state: open === undefined ? reading.state : open.value === true ? "door-open" : "door-shut",
      view: { ...reading.view, opening: open?.value === true ? 1 : 0 },
    };
  },
  draw: (view) => {
    const [hx, hy] = DOOR.hinge;
    const angle = ((view.opening ?? 0) * DOOR.opens * Math.PI) / 180;
    const end = [round(hx + DOOR.length * Math.sin(angle)), round(hy - DOOR.length * Math.cos(angle))];
    const open = (view.opening ?? 0) > 0;
    return [
      el("path", { "data-part": "body", d: "M6 6 H78 V58 H6 Z", fill: view.unknown ? HATCH : BODY, stroke: "none" }),
      // Insulated walls, open where the door is.
      el("path", {
        "data-part": "walls",
        d: "M78 26 V6 H6 V58 H78 V46 M74 26 V10 H10 V54 H74 V46 M74 26 H78 M74 46 H78",
        fill: "none",
        ...LINE,
      }),
      // The snowflake that marks a cold room.
      el("path", {
        "data-part": "cold",
        d: "M30 23 V41 M22.2 27.5 L37.8 36.5 M22.2 36.5 L37.8 27.5",
        fill: "none",
        ...LINE,
      }),
      ...(view.unknown
        ? []
        : [
            ...(open
              ? [
                  el("path", {
                    "data-part": "swing",
                    d: `M${hx} ${hy - DOOR.length} A${DOOR.length} ${DOOR.length} 0 0 1 ${end[0]} ${end[1]}`,
                    fill: "none",
                    ...LINE,
                    "stroke-dasharray": "3 3",
                  }),
                ]
              : []),
            el("path", {
              "data-part": "door",
              "data-open": String(open),
              d: `M${hx} ${hy} L${end[0]} ${end[1]}`,
              fill: "none",
              stroke: open ? MOVING : STILL,
              "stroke-width": 3,
              "stroke-linecap": "round",
              "vector-effect": "non-scaling-stroke",
            }),
          ]),
    ];
  },
};

export const REFRIGERATION_UNIT: SymbolDefinition = {
  size: [96, 64],
  ports: {},
  properties: ["running", "fault", "manual", "out-of-service"],
  deciding: ["fault", "running"],
  states: runningStates(),
  sample: "running",
  read: readRunning,
  rules: [{ part: "fan", spin: "running" }],
  draw: (view) => [
    el("rect", {
      "data-part": "housing",
      x: 4,
      y: 8,
      width: 88,
      height: 48,
      rx: 4,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    // The compressor (ISO 10628-2): a circle with its two lines narrowing in the flow's direction.
    el("circle", { "data-part": "compressor", cx: 28, cy: 32, r: 15, fill: BODY, ...LINE }),
    el("path", { d: "M15.5 24 L40 28.5 M15.5 40 L40 35.5", fill: "none", ...LINE }),
    el("circle", { "data-part": "shroud", cx: 66, cy: 32, r: 15, fill: BODY, ...LINE }),
    ...(view.unknown ? [] : [fan(66, 32, 13, view.parts?.fan?.spin)]),
  ],
};

export const HEATER: SymbolDefinition = {
  size: [64, 64],
  ports: {
    inlet: { at: [20, 0], direction: "up" },
    outlet: { at: [64, 16], direction: "right" },
  },
  properties: ["power", "temperature", "level"],
  deciding: ["power", "temperature"],
  states: [
    {
      id: "heating",
      name: "Heating",
      values: { power: 40, temperature: { share: 0.55 }, level: 80 },
      needs: ["power"],
    },
    { id: "idle", name: "Idle", values: { power: 0, temperature: { share: 0.3 }, level: 80 }, needs: ["power"] },
  ],
  sample: "heating",
  read: readTemperature,
  rules: [
    { part: "element", active: "power" },
    { part: "liquid", level: "level", range: [0, 100] },
  ],
  outline: roundedRectOutline(HEATER_VESSEL),
  draw: (view) => {
    const active = view.parts?.element?.active === true;
    return [
      el("rect", { "data-part": "body", ...HEATER_VESSEL, fill: view.unknown ? HATCH : BODY, stroke: "none" }),
      ...(view.unknown
        ? []
        : roundedLiquid("liquid", { x: 9, y: 7, width: 46, height: 50 }, view.parts?.liquid?.level, 5)),
      el("rect", { "data-part": "vessel", ...HEATER_VESSEL, fill: "none", ...LINE }),
      el("path", { "data-part": "pipes", d: "M20 0 V6 M57 16 H64", fill: "none", ...LINE }),
      // The heating element: a coil across the bottom, its leads through the wall.
      el("path", {
        "data-part": "element",
        "data-active": String(active),
        d: "M8 48 H13 L17 42 L21 50 L25 42 L29 50 L33 42 L37 50 L41 42 L45 50 L49 42 L51 46",
        fill: "none",
        stroke: active ? MOVING : STILL,
        "stroke-width": 2.5,
        "stroke-linejoin": "round",
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke",
      }),
      ...(active && !view.unknown ? [heat([22, 32, 42], 37)] : []),
    ];
  },
};

export const COOLER: SymbolDefinition = {
  size: [64, 64],
  ports: {
    inlet: { at: [0, 18], direction: "left" },
    outlet: { at: [64, 18], direction: "right" },
  },
  properties: ["running", "fault", "manual", "out-of-service"],
  deciding: ["fault", "running"],
  states: runningStates(),
  sample: "running",
  read: readRunning,
  rules: [{ part: "fan", spin: "running" }],
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M0 18 H6 M58 18 H64", fill: "none", ...LINE }),
    // The tube bundle the air cools, and its fan beneath (ISO 10628-2 air cooler).
    el("rect", {
      "data-part": "bundle",
      x: 6,
      y: 10,
      width: 52,
      height: 16,
      rx: 2,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    el("path", { d: "M11 15 H53 M11 21 H53", fill: "none", ...LINE }),
    el("circle", { "data-part": "shroud", cx: 32, cy: 45, r: 14, fill: BODY, ...LINE }),
    el("path", { d: "M32 31 V26", fill: "none", ...LINE }),
    ...(view.unknown ? [] : [fan(32, 45, 12, view.parts?.fan?.spin)]),
  ],
};

export const JACKETED_VESSEL: SymbolDefinition = {
  size: [64, 64],
  ports: {
    inlet: { at: [32, 0], direction: "up" },
    outlet: { at: [32, 64], direction: "down" },
    "jacket-inlet": { at: [0, 52], direction: "left" },
    "jacket-outlet": { at: [64, 26], direction: "right" },
  },
  properties: ["level", "temperature"],
  deciding: ["level", "temperature"],
  states: [
    {
      id: "filled",
      name: "Filled",
      values: { level: 60, temperature: { share: 0.5 } },
      needs: ["level|temperature"],
    } satisfies SymbolState,
  ],
  sample: "filled",
  read: readTemperature,
  rules: [{ part: "liquid", level: "level", range: [0, 100] }],
  outline: `${JACKET} ${roundedRectOutline(JACKETED)}`,
  draw: (view) => [
    // The jacket around the lower part of the vessel, then the vessel over it.
    el("path", { "data-part": "jacket", d: JACKET, fill: BODY, ...LINE }),
    el("path", { "data-part": "pipes", d: "M32 0 V6 M32 61 V64 M0 52 H9 M55 26 H64", fill: "none", ...LINE }),
    el("rect", { "data-part": "body", ...JACKETED, fill: view.unknown ? HATCH : BODY, stroke: "none" }),
    ...(view.unknown
      ? []
      : roundedLiquid("liquid", { x: 17, y: 7, width: 30, height: 48 }, view.parts?.liquid?.level, 7)),
    el("rect", { "data-part": "vessel", ...JACKETED, fill: "none", ...LINE }),
  ],
};

export const COUPLED_VESSELS: SymbolDefinition = {
  size: [96, 64],
  ports: {
    "inlet-1": { at: [20, 0], direction: "up" },
    "outlet-1": { at: [20, 64], direction: "down" },
    "inlet-2": { at: [76, 0], direction: "up" },
    "outlet-2": { at: [76, 64], direction: "down" },
  },
  properties: ["temperature-1", "temperature-2"],
  deciding: ["temperature-1", "temperature-2"],
  states: [
    {
      id: "left-warmer",
      name: "Left warmer",
      values: { "temperature-1": { share: 0.7 }, "temperature-2": { share: 0.4 } },
      needs: ["temperature-1|temperature-2"],
    },
    {
      id: "even",
      name: "Even",
      values: { "temperature-1": { share: 0.5 }, "temperature-2": { share: 0.5 } },
      needs: ["temperature-1|temperature-2"],
    },
  ],
  sample: "left-warmer",
  // The heat passes the wall from the warmer vessel to the colder, when they differ by at least a
  // hundredth of the object's range.
  read: (value, frame, faulted, object) => {
    const reading = readNumbers("temperature-1", "temperature-2")(value, frame, faulted, object);
    const [left, right] = [Number(value("temperature-1")?.value), Number(value("temperature-2")?.value)];
    const least = ((object.max ?? 100) - (object.min ?? 0)) / 100;
    if (!Number.isFinite(left) || !Number.isFinite(right) || Math.abs(left - right) < least) return reading;
    return { ...reading, view: { ...reading.view, direction: left > right ? "right" : "left" } };
  },
  outline: roundedRectOutline(COUPLED),
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M20 0 V6 M20 58 V64 M76 0 V6 M76 58 V64", fill: "none", ...LINE }),
    el("rect", { "data-part": "vessels", ...COUPLED, fill: view.unknown ? HATCH : BODY, ...LINE }),
    // The wall the heat passes through, drawn heavy.
    el("path", {
      "data-part": "wall",
      d: "M48 7 V57",
      fill: "none",
      stroke: LINE.stroke,
      "stroke-width": 4,
      "vector-effect": "non-scaling-stroke",
    }),
    ...(view.direction && !view.unknown ? [heatFlow(view.direction)] : []),
  ],
};

/** Three arrows through the wall at x 48, the way the heat passes. */
function heatFlow(direction: "left" | "right"): SvgNode {
  const [from, to, back] = direction === "right" ? [40, 56, 52] : [56, 40, 44];
  const arrows = [20, 32, 44]
    .map((y) => `M${from} ${y} H${to} M${back} ${y - 3} L${to} ${y} L${back} ${y + 3}`)
    .join(" ");
  return el("path", {
    "data-part": "heat-flow",
    "data-direction": direction,
    d: arrows,
    fill: "none",
    stroke: MOVING,
    "stroke-width": 1.5,
    "stroke-linecap": "round",
    "stroke-linejoin": "round",
    "vector-effect": "non-scaling-stroke",
  });
}

export const HEAT_DRAW_OFF: SymbolDefinition = {
  size: [64, 64],
  ports: {
    inlet: { at: [0, 44], direction: "left" },
    outlet: { at: [64, 44], direction: "right" },
  },
  properties: ["power"],
  deciding: ["power"],
  states: [
    { id: "drawing", name: "Drawing heat", values: { power: { share: 0.4 } }, needs: ["power"] },
    { id: "idle", name: "Idle", values: { power: 0 }, needs: ["power"] },
  ],
  sample: "drawing",
  read: (value, frame, faulted, object) => ({
    ...readNumbers("power")(value, frame, faulted, object),
    word: shownBoundNumber(value, "power", frame, object) ?? "",
  }),
  rules: [{ part: "heat", active: "power" }],
  draw: (view) => [
    // A finned radiator the heat leaves through, and the heat rising from it while it draws.
    el("path", { "data-part": "pipes", d: "M0 44 H12 M52 44 H64", fill: "none", ...LINE }),
    el("path", {
      "data-part": "fins",
      d: "M17 39 V31 M23 39 V31 M29 39 V31 M35 39 V31 M41 39 V31 M47 39 V31",
      fill: "none",
      ...LINE,
    }),
    el("rect", {
      "data-part": "radiator",
      x: 12,
      y: 39,
      width: 40,
      height: 10,
      rx: 2,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    ...(view.parts?.heat?.active && !view.unknown ? [heat([20, 32, 44], 26)] : []),
  ],
};
