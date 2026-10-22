// The symbols of the equipment PLC programmers need (specification 39, §8.8; specification 41,
// §7 and §8): the fan, the blower, the agitator and the conveyor, which turn while they run; the
// damper, the cylinder and the motor valve, which stand open or closed; and the weigh scale.

import { token } from "../parts";
import { el, SvgNode, text } from "../svg";
import type { SymbolArt, SymbolState, SymbolView } from "./index";
import { percentWord, readNumbers, readRunning, readRunningWith, runningStates } from "./reading";
import { BODY, FILLED, HATCH, LINE, MOVING, round, STILL } from "./style";
import { AXIS, BODY_PATH, MIDDLE, readFeedback, STUBS_PATH } from "./valve";

/** A part that turns about (`x`, `y`) while the machine runs: its span is the whole circle. */
export function rotor(
  x: number,
  y: number,
  radius: number,
  view: SymbolView,
  children: SvgNode[],
  part = "rotor",
): SvgNode {
  return el(
    "g",
    {
      "data-part": part,
      class: view.turning ? "hmi-spin" : undefined,
      style: view.turning ? `animation-delay: -${view.turning.delayMs}ms` : undefined,
    },
    [el("circle", { cx: x, cy: y, r: radius, fill: "none", stroke: "none" }), ...children],
  );
}

/** A moving part's colour: the brightest while it moves. */
export const paint = (view: SymbolView): string => (view.turning ? MOVING : STILL);

/** The ISO 14617 arrow of variability, behind the symbol. */
const VARIABLE = el("path", {
  "data-part": "variable",
  d: "M3 61 L59 5 M59 5 L51.3 7.1 M59 5 L56.9 12.7",
  fill: "none",
  ...LINE,
});

const MACHINE = {
  properties: ["running", "fault", "manual", "out-of-service"],
  deciding: ["fault", "running"],
  sample: "running",
  read: readRunning,
  states: runningStates(),
};

/** A fan: three blades in its housing; the variable-speed variant gives its speed. */
export const FAN: SymbolArt = {
  ...MACHINE,
  properties: [...MACHINE.properties, "speed"],
  read: readRunningWith("speed", true),
  states: runningStates((running) => ({ speed: running ? 62 : 0 })),
  draw: (view) => [
    ...(view.variant === "variable-speed" ? [VARIABLE] : []),
    el("circle", { "data-part": "housing", cx: 32, cy: 32, r: 26, fill: view.unknown ? HATCH : BODY, ...LINE }),
    ...(view.unknown
      ? []
      : [
          rotor(32, 32, 20, view, [
            ...[0, 120, 240].map((angle) =>
              el("ellipse", {
                cx: 32,
                cy: 21,
                rx: 4.5,
                ry: 10,
                fill: paint(view),
                transform: `rotate(${angle} 32 32)`,
              }),
            ),
            el("circle", { cx: 32, cy: 32, r: 3.5, fill: BODY, ...LINE }),
          ]),
        ]),
  ],
};

/** A blower (ISO 14617 compressor): a circle with its converging lines, its rotor's mark circling. */
export const BLOWER: SymbolArt = {
  ...MACHINE,
  draw: (view) => [
    el("path", { d: "M0 32 H6 M58 32 H64", fill: "none", ...LINE }),
    el("circle", { "data-part": "casing", cx: 32, cy: 32, r: 26, fill: view.unknown ? HATCH : BODY, ...LINE }),
    el("path", { d: "M10 18 L54 26 M10 46 L54 38", fill: "none", ...LINE }),
    ...(view.unknown ? [] : [rotor(32, 32, 20, view, [el("circle", { cx: 32, cy: 14, r: 3, fill: paint(view) })])]),
  ],
};

/** An agitator: its motor on top, its shaft and its impeller, which turns while it runs. */
export const AGITATOR: SymbolArt = {
  ...MACHINE,
  draw: (view) => [
    el("rect", {
      "data-part": "motor",
      x: 20,
      y: 2,
      width: 24,
      height: 15,
      rx: 3,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    text("M", { x: 32, y: 14, fill: token("muted"), "font-size": 11, "font-weight": 600, "text-anchor": "middle" }),
    el("path", {
      "data-part": "shaft",
      d: "M32 17 V41",
      fill: "none",
      stroke: LINE.stroke,
      "stroke-width": 3,
      "stroke-linecap": "round",
      "vector-effect": "non-scaling-stroke",
    }),
    ...(view.unknown
      ? []
      : [
          rotor(32, 50, 12, view, [
            ...[0, 90].map((angle) =>
              el("ellipse", {
                cx: 32,
                cy: 50,
                rx: 12,
                ry: 3.5,
                fill: paint(view),
                transform: `rotate(${angle} 32 50)`,
              }),
            ),
            el("circle", { cx: 32, cy: 50, r: 3, fill: BODY, ...LINE }),
          ]),
        ]),
  ],
};

/** A roller of the conveyor: its spokes turn while it runs. */
function roller(x: number, view: SymbolView, part: string): SvgNode {
  return rotor(
    x,
    20,
    9,
    view,
    [
      el("path", {
        d: `M${x - 6} 20 H${x + 6} M${x} 14 V26`,
        fill: "none",
        stroke: paint(view),
        "stroke-width": 2,
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke",
      }),
    ],
    part,
  );
}

/** A belt conveyor: the belt around its two rollers, on its legs. */
export const CONVEYOR: SymbolArt = {
  ...MACHINE,
  draw: (view) => [
    el("path", { "data-part": "legs", d: "M14 29 L10 39 M82 29 L86 39", fill: "none", ...LINE }),
    el("rect", {
      "data-part": "belt",
      x: 4,
      y: 10,
      width: 88,
      height: 20,
      rx: 10,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    el("circle", { cx: 14, cy: 20, r: 9, fill: BODY, ...LINE }),
    el("circle", { cx: 82, cy: 20, r: 9, fill: BODY, ...LINE }),
    ...(view.unknown ? [] : [roller(14, view, "rotor"), roller(82, view, "rotor-2")]),
  ],
};

type Flag = "fault" | "manual" | "out-of-service";

/** A state of a two-position actuator: its feedbacks, a position, and its flags. */
function standing(
  id: string,
  name: string,
  open: boolean,
  closed: boolean,
  extra: Record<string, unknown> = {},
  flags: Partial<Record<Flag, boolean>> = {},
): SymbolState {
  const set = Object.keys(flags);
  return {
    id,
    name,
    values: {
      open,
      closed,
      fault: false,
      manual: false,
      "out-of-service": false,
      ...extra,
      ...flags,
    } as SymbolState["values"],
    needs: set.length > 0 ? set : ["open|closed"],
  };
}

const STANDING = (extra: (open: boolean) => Record<string, unknown> = () => ({})): SymbolState[] => [
  standing("open", "Open", true, false, extra(true)),
  standing("closed", "Closed", false, true, extra(false)),
  standing("moving", "Moving", false, false, extra(false)),
  standing("fault", "Fault", false, true, extra(false), { fault: true }),
  standing("manual", "Manual", true, false, extra(true), { manual: true }),
  standing("out-of-service", "Out of service", false, true, extra(false), { "out-of-service": true }),
];

const POSITIONED = ["open", "closed", "fault", "manual", "out-of-service"];

/** A damper in its duct: the blade lies along the duct when open and across it when closed; a
 * modulating damper's blade turns with its position. */
export const DAMPER: SymbolArt = {
  properties: [...POSITIONED, "position"],
  deciding: ["fault", "open", "closed", "position"],
  sample: "open",
  states: STANDING((open) => ({ position: { share: open ? 1 : 0 } })).map((state) =>
    state.id === "moving" ? { ...state, values: { ...state.values, position: { share: 0.5 } } } : state,
  ),
  read: (value, frame, faulted, object) => {
    const position = value("position");
    if (!position) return readFeedback(value("open"), value("closed"), faulted);
    const number = Number(position.value);
    const share = Number.isFinite(number) ? Math.min(1, Math.max(0, number / 100)) : 0;
    const numbers = readNumbers("position")(value, frame, faulted, object);
    return {
      ...numbers,
      word: faulted ? numbers.word : (percentWord(position) ?? ""),
      view: { unknown: false, opening: share },
    };
  },
  draw: (view) => {
    const angle = round(90 * (1 - (view.opening ?? 0)));
    return [
      el("path", { d: "M0 32 H4 M60 32 H64", fill: "none", ...LINE }),
      el("rect", {
        "data-part": "duct",
        x: 4,
        y: 14,
        width: 56,
        height: 36,
        rx: 2,
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
      }),
      ...(view.unknown
        ? []
        : [
            el("path", {
              "data-part": "blade",
              "data-angle": angle,
              d: `M${MIDDLE - 15} 32 H${MIDDLE + 15}`,
              fill: "none",
              stroke: MOVING,
              "stroke-width": 3.5,
              "stroke-linecap": "round",
              "vector-effect": "non-scaling-stroke",
              transform: angle === 0 ? undefined : `rotate(${angle} ${MIDDLE} 32)`,
            }),
          ]),
      el("circle", { cx: MIDDLE, cy: 32, r: 3, fill: BODY, ...LINE }),
    ];
  },
};

/** A cylinder: its piston and rod run out when extended and back when retracted. */
export const CYLINDER: SymbolArt = {
  properties: POSITIONED,
  deciding: ["fault", "open", "closed"],
  sample: "closed",
  states: STANDING(),
  read: (value, _frame, faulted) =>
    readFeedback(value("open"), value("closed"), faulted, { open: "Extended", closed: "Retracted" }),
  draw: (view) => {
    const travel = view.opening ?? 0;
    return [
      el("rect", {
        "data-part": "barrel",
        x: 4,
        y: 8,
        width: 52,
        height: 24,
        rx: 3,
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
      }),
      el("path", { d: "M12 8 V4 M48 8 V4", fill: "none", ...LINE }),
      ...(view.unknown
        ? []
        : [
            el(
              "g",
              {
                "data-part": "rod",
                "data-travel": travel,
                transform: travel === 0 ? undefined : `translate(${round(36 * travel)} 0)`,
              },
              [
                el("rect", { x: 8, y: 11, width: 5, height: 18, rx: 1, fill: MOVING }),
                el("rect", { x: 13, y: 18, width: 43, height: 4, rx: 1, fill: MOVING }),
              ],
            ),
          ]),
    ];
  },
};

/** A motor valve: the valve body and its stem with the motor (IEC 60617, M) on top; the body is
 * filled when closed. */
export const MOTOR_VALVE: SymbolArt = {
  properties: POSITIONED,
  deciding: ["fault", "open", "closed"],
  sample: "open",
  states: STANDING(),
  read: (value, _frame, faulted) => readFeedback(value("open"), value("closed"), faulted),
  draw: (view) => [
    el("path", { d: STUBS_PATH, fill: "none", ...LINE }),
    el("path", { "data-part": "stem", d: `M${MIDDLE} ${AXIS} V23`, fill: "none", ...LINE }),
    el("g", { "data-part": "motor" }, [
      el("circle", { cx: MIDDLE, cy: 13, r: 10, fill: BODY, ...LINE }),
      text("M", {
        x: MIDDLE,
        y: 17.5,
        fill: token("muted"),
        "font-size": 12,
        "font-weight": 600,
        "text-anchor": "middle",
      }),
    ]),
    el("path", {
      "data-part": "body",
      "data-filled": String(view.filled === true),
      d: BODY_PATH,
      fill: view.unknown ? HATCH : view.filled ? FILLED : BODY,
      ...LINE,
    }),
  ],
};

/** A weigh scale: its platform on the load cell; the word gives the weight. */
export const WEIGH_SCALE: SymbolArt = {
  properties: ["value", "fault"],
  deciding: ["fault", "value"],
  sample: "weighing",
  states: [
    { id: "weighing", name: "Weighing", values: { value: { share: 0.6 }, fault: false }, needs: ["value"] },
    { id: "fault", name: "Fault", values: { value: { share: 0.6 }, fault: true }, needs: ["fault"] },
  ],
  read: readNumbers("value"),
  draw: (view) => [
    el("rect", {
      "data-part": "platform",
      x: 6,
      y: 26,
      width: 52,
      height: 7,
      rx: 2,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    el("rect", { "data-part": "load-cell", x: 27, y: 33, width: 10, height: 13, rx: 1, fill: MOVING }),
    el("path", { "data-part": "base", d: "M12 60 L18 46 H46 L52 60 Z", fill: BODY, ...LINE }),
  ],
};
