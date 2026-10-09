// Motion equipment (specification 39, §5.2 phase 2, motion; §5.3 items 4 and 5): the servo motor
// with its drive, the shaft, the drum with its hanging load, the flexible coupling with its load,
// the encoder, the rotary arm and the axis position indicator. What turns stands at its angle:
// 0° straight up and clockwise, as on a dial; the rotary arm hangs straight down at 0°, its rest.

import { token } from "../parts";
import { el, SvgNode, text } from "../svg";
import type { SymbolDefinition, SymbolState } from "./index";
import { readNumbers, readRunning, readRunningWith, runningStates, shownBoundNumber, SymbolReader } from "./reading";
import { BODY, HATCH, LINE, MOVING, round, STILL } from "./style";

/** A turning part: stood at its angle about (`x`, `y`), or turning while it runs. */
function turned(
  part: string,
  x: number,
  y: number,
  children: SvgNode[],
  angle: number | undefined,
  spin?: { delayMs: number },
): SvgNode {
  if (angle !== undefined)
    return el(
      "g",
      { "data-part": part, "data-angle": round(angle), transform: `rotate(${round(angle)} ${x} ${y})` },
      children,
    );
  return el(
    "g",
    {
      "data-part": part,
      class: spin ? "hmi-spin" : undefined,
      style: spin ? `animation-delay: -${spin.delayMs}ms` : undefined,
    },
    children,
  );
}

/** A marker from (`x`, `y`) up to `length`: the line a turning part is read by. */
function marker(x: number, y: number, length: number, paint = MOVING, width = 2.5): SvgNode {
  return el("path", {
    d: `M${x} ${y} V${y - length}`,
    fill: "none",
    stroke: paint,
    "stroke-width": width,
    "stroke-linecap": "round",
    "vector-effect": "non-scaling-stroke",
  });
}

/** A state of a symbol driven by its numbers. */
function state(id: string, name: string, values: SymbolState["values"]): SymbolState {
  return { id, name, values, needs: [Object.keys(values).join("|")] };
}

/** The word of an angle, else of a speed. */
const readAngle: SymbolReader = (value, frame, faulted, object) => {
  const reading = readNumbers("angle")(value, frame, faulted, object);
  return reading.word || faulted
    ? reading
    : { ...reading, word: shownBoundNumber(value, "speed", frame, object) ?? "" };
};

export const MOTOR: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["running", "fault", "manual", "out-of-service"],
  deciding: ["fault", "running"],
  states: runningStates(),
  sample: "running",
  read: readRunning,
  rules: [{ part: "rotor", spin: "running" }],
  draw: (view) => {
    const rotor = view.parts?.rotor;
    return [
      // Its shaft leaves to the right.
      el("path", {
        "data-part": "shaft",
        d: "M50 32 H62",
        fill: "none",
        stroke: LINE.stroke,
        "stroke-width": 4,
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke",
      }),
      // IEC 60617: a circle with M, its rotor's mark circling inside it while it runs.
      el("circle", { "data-part": "motor", cx: 28, cy: 32, r: 22, fill: view.unknown ? HATCH : BODY, ...LINE }),
      text("M", { x: 28, y: 38.5, fill: token("muted"), "font-size": 18, "font-weight": 600, "text-anchor": "middle" }),
      ...(view.unknown
        ? []
        : [
            turned(
              "rotor",
              28,
              32,
              [
                el("circle", { cx: 28, cy: 32, r: 22, fill: "none", stroke: "none" }),
                el("circle", { cx: 28, cy: 14.5, r: 3, fill: rotor?.spin ? MOVING : STILL }),
              ],
              undefined,
              rotor?.spin,
            ),
          ]),
    ];
  },
};

export const SERVO_MOTOR: SymbolDefinition = {
  size: [96, 64],
  ports: {},
  properties: ["running", "fault", "manual", "out-of-service", "speed", "angle"],
  deciding: ["fault", "running"],
  states: runningStates((running) => ({ speed: running ? { share: 0.5 } : 0, angle: running ? 30 : 0 })),
  sample: "running",
  read: readRunningWith("speed", false),
  rules: [
    { part: "rotor", angle: "angle" },
    { part: "rotor", spin: "running" },
  ],
  draw: (view) => {
    const rotor = view.parts?.rotor;
    const moving = rotor?.angle !== undefined || rotor?.spin !== undefined;
    return [
      // The drive (IEC 60617 converter: a box with its diagonal, alternating and direct current).
      el("rect", {
        "data-part": "drive",
        x: 4,
        y: 14,
        width: 30,
        height: 36,
        rx: 3,
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
      }),
      el("path", {
        d: "M8 46 L30 18 M9 24 q2 -3 4 0 q2 3 4 0 M22 40 H28 M22 43.5 H28 M34 32 H44 M75 32 H92",
        fill: "none",
        ...LINE,
      }),
      // The motor (IEC 60617: a circle with M), its rotor's mark circling inside it.
      el("circle", { "data-part": "motor", cx: 60, cy: 32, r: 15, fill: view.unknown ? HATCH : BODY, ...LINE }),
      text("M", { x: 60, y: 37, fill: token("muted"), "font-size": 14, "font-weight": 600, "text-anchor": "middle" }),
      ...(view.unknown
        ? []
        : [
            turned(
              "rotor",
              60,
              32,
              [
                el("circle", { cx: 60, cy: 32, r: 15, fill: "none", stroke: "none" }),
                el("circle", { cx: 60, cy: 21, r: 2.5, fill: moving ? MOVING : STILL }),
              ],
              rotor?.angle,
              rotor?.spin,
            ),
          ]),
    ];
  },
};

export const SHAFT: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["angle", "speed"],
  deciding: ["angle", "speed"],
  states: [
    state("turning", "Turning", { angle: 30, speed: { share: 0.5 } }),
    state("standing", "Standing", { angle: 0, speed: 0 }),
  ],
  sample: "turning",
  read: readAngle,
  rules: [
    { part: "marker", angle: "angle" },
    { part: "marker", spin: "speed" },
  ],
  draw: (view) => {
    const mark = view.parts?.marker;
    return [
      el("path", {
        "data-part": "shaft",
        d: "M2 32 H42",
        fill: "none",
        stroke: LINE.stroke,
        "stroke-width": 4,
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke",
      }),
      el("rect", { "data-part": "bearings", x: 10, y: 24, width: 8, height: 16, rx: 2, fill: BODY, ...LINE }),
      el("rect", { x: 28, y: 24, width: 8, height: 16, rx: 2, fill: BODY, ...LINE }),
      el("circle", { "data-part": "end", cx: 50, cy: 32, r: 11, fill: view.unknown ? HATCH : BODY, ...LINE }),
      ...(view.unknown
        ? []
        : [
            turned(
              "marker",
              50,
              32,
              [
                el("circle", { cx: 50, cy: 32, r: 11, fill: "none", stroke: "none" }),
                marker(50, 32, 9, mark ? MOVING : STILL),
              ],
              mark?.angle,
              mark?.spin,
            ),
          ]),
    ];
  },
};

export const DRUM: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["angle", "position"],
  deciding: ["angle", "position"],
  states: [
    state("raised", "Raised", { angle: 0, position: { share: 0.2 } }),
    state("lowered", "Lowered", { angle: 270, position: { share: 0.8 } }),
  ],
  sample: "raised",
  read: (value, frame, faulted, object) => ({ ...readNumbers("position")(value, frame, faulted, object) }),
  rules: [
    { part: "spokes", angle: "angle" },
    { part: "load", travel: "position" },
  ],
  draw: (view) => {
    const travel = view.parts?.load?.travel ?? 0.3;
    const top = round(30 + travel * 22);
    return [
      el("path", { "data-part": "support", d: "M14 3 H50 M32 3 V7", fill: "none", ...LINE }),
      el("circle", { "data-part": "drum", cx: 32, cy: 18, r: 12, fill: view.unknown ? HATCH : BODY, ...LINE }),
      ...(view.unknown
        ? []
        : [
            turned(
              "spokes",
              32,
              18,
              [
                marker(32, 18, 10),
                el("path", {
                  d: "M32 18 L40.7 23 M32 18 L23.3 23",
                  fill: "none",
                  stroke: MOVING,
                  "stroke-width": 2.5,
                  "stroke-linecap": "round",
                  "vector-effect": "non-scaling-stroke",
                }),
              ],
              view.parts?.spokes?.angle ?? 0,
            ),
            el("path", { "data-part": "rope", d: `M44 18 V${top}`, fill: "none", ...LINE }),
            el("rect", {
              "data-part": "load",
              "data-travel": round(travel),
              x: 35,
              y: top,
              width: 18,
              height: 10,
              rx: 2,
              fill: MOVING,
            }),
          ]),
    ];
  },
};

export const COUPLING: SymbolDefinition = {
  size: [96, 64],
  ports: {},
  properties: ["motor-angle", "load-angle"],
  deciding: ["motor-angle", "load-angle"],
  states: [
    state("twisted", "Twisted", { "motor-angle": 40, "load-angle": 32 }),
    state("aligned", "Aligned", { "motor-angle": 40, "load-angle": 40 }),
  ],
  sample: "twisted",
  read: readNumbers("motor-angle", "load-angle"),
  rules: [
    { part: "motor-marker", angle: "motor-angle" },
    { part: "load-marker", angle: "load-angle" },
  ],
  draw: (view) => [
    el("path", {
      "data-part": "shafts",
      d: "M0 32 H8 M88 32 H96",
      fill: "none",
      stroke: LINE.stroke,
      "stroke-width": 4,
      "vector-effect": "non-scaling-stroke",
    }),
    // The flexible element between the motor's side and the load's.
    el("path", { "data-part": "spring", d: "M32 32 L36 26 L42 38 L48 26 L54 38 L60 26 L64 32", fill: "none", ...LINE }),
    el("circle", { "data-part": "motor-side", cx: 20, cy: 32, r: 12, fill: view.unknown ? HATCH : BODY, ...LINE }),
    el("circle", { "data-part": "load-side", cx: 76, cy: 32, r: 12, fill: view.unknown ? HATCH : BODY, ...LINE }),
    ...(view.unknown
      ? []
      : [
          turned("motor-marker", 20, 32, [marker(20, 32, 10)], view.parts?.["motor-marker"]?.angle ?? 0),
          turned("load-marker", 76, 32, [marker(76, 32, 10)], view.parts?.["load-marker"]?.angle ?? 0),
        ]),
  ],
};

export const ENCODER: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["angle"],
  deciding: ["angle"],
  states: [state("zero", "At 0°", { angle: 0 }), state("turned", "Turned", { angle: 135 })],
  sample: "turned",
  read: readNumbers("angle"),
  rules: [{ part: "disc", angle: "angle" }],
  draw: (view) => {
    // The code wheel's ticks, its index mark the longest.
    const ticks = Array.from({ length: 12 }, (_, index) => {
      const angle = (index * 30 * Math.PI) / 180;
      const [inner, outer] = [index === 0 ? 6 : 12, 16];
      return `M${round(28 + inner * Math.sin(angle))} ${round(32 - inner * Math.cos(angle))} L${round(28 + outer * Math.sin(angle))} ${round(32 - outer * Math.cos(angle))}`;
    }).join(" ");
    return [
      el("path", {
        "data-part": "shaft",
        d: "M0 32 H10",
        fill: "none",
        stroke: LINE.stroke,
        "stroke-width": 4,
        "vector-effect": "non-scaling-stroke",
      }),
      el("circle", { "data-part": "wheel", cx: 28, cy: 32, r: 19, fill: view.unknown ? HATCH : BODY, ...LINE }),
      ...(view.unknown
        ? []
        : [
            turned(
              "disc",
              28,
              32,
              [
                el("path", {
                  d: ticks,
                  fill: "none",
                  stroke: MOVING,
                  "stroke-width": 1.5,
                  "stroke-linecap": "round",
                  "vector-effect": "non-scaling-stroke",
                }),
              ],
              view.parts?.disc?.angle ?? 0,
            ),
          ]),
      // The read head over the wheel's rim.
      el("rect", { "data-part": "head", x: 43, y: 25, width: 14, height: 14, rx: 2, fill: BODY, ...LINE }),
    ];
  },
};

export const ROTARY_ARM: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["angle"],
  deciding: ["angle"],
  states: [state("hanging", "Hanging", { angle: 0 }), state("raised", "Raised", { angle: 60 })],
  sample: "raised",
  read: readNumbers("angle"),
  rules: [{ part: "arm", angle: "angle" }],
  // The joint in the middle, held from the wall at the left, so the arm can turn all the way
  // round within the symbol.
  draw: (view) => [
    el("path", { "data-part": "mount", d: "M5 20 V40 M5 30 H28", fill: "none", ...LINE }),
    // Gravity, against which the joint works.
    el("path", { "data-part": "gravity", d: "M57 44 V60 M53.5 56.5 L57 60 L60.5 56.5", fill: "none", ...LINE }),
    ...(view.unknown
      ? [el("circle", { cx: 32, cy: 30, r: 4, fill: HATCH, ...LINE })]
      : [
          turned(
            "arm",
            32,
            30,
            [
              el("path", {
                d: "M32 30 V46",
                fill: "none",
                stroke: MOVING,
                "stroke-width": 3,
                "stroke-linecap": "round",
                "vector-effect": "non-scaling-stroke",
              }),
              el("circle", { cx: 32, cy: 50, r: 6, fill: MOVING }),
            ],
            view.parts?.arm?.angle ?? 0,
          ),
          el("circle", { "data-part": "pivot", cx: 32, cy: 30, r: 4, fill: BODY, ...LINE }),
        ]),
  ],
};

export const AXIS_INDICATOR: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["actual", "target"],
  deciding: ["actual", "target"],
  states: [
    state("on-target", "On target", { actual: 90, target: 90 }),
    state("following", "Following", { actual: 75, target: 90 }),
  ],
  sample: "following",
  // The actual angle, and the target beside it while the axis has not reached it.
  read: (value, frame, faulted, object) => {
    const reading = readNumbers("actual")(value, frame, faulted, object);
    const [actual, target] = [
      shownBoundNumber(value, "actual", frame, object),
      shownBoundNumber(value, "target", frame, object),
    ];
    return !faulted && actual && target && actual !== target ? { ...reading, word: `${actual} → ${target}` } : reading;
  },
  rules: [
    { part: "actual", angle: "actual" },
    { part: "target", angle: "target" },
  ],
  draw: (view) => {
    const ticks = Array.from({ length: 12 }, (_, index) => {
      const angle = (index * 30 * Math.PI) / 180;
      return `M${round(32 + 22 * Math.sin(angle))} ${round(32 - 22 * Math.cos(angle))} L${round(32 + 26 * Math.sin(angle))} ${round(32 - 26 * Math.cos(angle))}`;
    }).join(" ");
    return [
      el("circle", { "data-part": "dial", cx: 32, cy: 32, r: 28, fill: view.unknown ? HATCH : BODY, ...LINE }),
      el("path", { "data-part": "ticks", d: ticks, fill: "none", ...LINE }),
      ...(view.unknown
        ? []
        : [
            // The target: a dashed line to a pointer on the rim.
            ...(view.parts?.target?.angle !== undefined
              ? [
                  turned(
                    "target",
                    32,
                    32,
                    [
                      el("path", { d: "M32 32 V10", fill: "none", ...LINE, "stroke-dasharray": "3 3" }),
                      el("path", { d: "M27.5 1 L36.5 1 L32 7.5 Z", fill: token("muted") }),
                    ],
                    view.parts.target.angle,
                  ),
                ]
              : []),
            turned("actual", 32, 32, [marker(32, 32, 21, MOVING, 3)], view.parts?.actual?.angle ?? 0),
            el("circle", { cx: 32, cy: 32, r: 3.5, fill: MOVING }),
          ]),
    ];
  },
};
