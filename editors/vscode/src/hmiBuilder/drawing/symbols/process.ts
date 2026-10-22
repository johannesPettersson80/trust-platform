// The process equipment of phase 3 (specification 39, §5.2), drawn as ISO 10628-2 draws it: the
// closed vessel with its dished heads, the heat exchanger, the filter, the cyclone, the crusher,
// whose hammer rotor turns while it runs, and the vibrating screen, whose exciter's weight circles.

import { token } from "../parts";
import { say } from "../speech";
import { el } from "../svg";
import type { SymbolArt } from "./index";
import { paint, rotor } from "./plant";
import { on, readNumbers, readRunning, readRunningWith, runningStates } from "./reading";
import { BODY, HATCH, LINE, LIQUID, round } from "./style";
import { levelStates } from "./tank";

const MACHINE = {
  properties: ["running", "fault", "manual", "out-of-service"],
  deciding: ["fault", "running"],
  sample: "running",
};

// The closed vessel: its shell between the tangent lines at y 14 and 50, each head a half ellipse.
const VESSEL_LEFT = 14;
const VESSEL_RIGHT = 50;
const HEAD_RX = 18;
const HEAD_RY = 10;
const VESSEL_TOP = 4;
const VESSEL_BOTTOM = 60;
const VESSEL_OUTLINE = `M${VESSEL_LEFT} 14 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_RIGHT} 14 V50 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_LEFT} 50 Z`;

/** Half the width of a head at height `y`, its centre at `centre`. */
function headHalfWidth(y: number, centre: number): number {
  const along = Math.min(1, Math.abs(y - centre) / HEAD_RY);
  return round(HEAD_RX * Math.sqrt(1 - along * along));
}

/** The liquid in the vessel at `level` (0 to 1): up to its surface, which may lie in a head. */
function vesselLiquid(level: number): string {
  const surface = round(VESSEL_BOTTOM - (VESSEL_BOTTOM - VESSEL_TOP) * Math.min(1, Math.max(0, level)));
  if (surface >= 50) {
    const half = headHalfWidth(surface, 50);
    return `M${round(32 - half)} ${surface} H${round(32 + half)} A${HEAD_RX} ${HEAD_RY} 0 0 1 ${round(32 - half)} ${surface} Z`;
  }
  const lower = `V50 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_LEFT} 50`;
  if (surface >= 14) return `M${VESSEL_LEFT} ${surface} H${VESSEL_RIGHT} ${lower} Z`;
  const half = headHalfWidth(surface, 14);
  return `M${round(32 - half)} ${surface} H${round(32 + half)} A${HEAD_RX} ${HEAD_RY} 0 0 1 ${VESSEL_RIGHT} 14 ${lower} V14 A${HEAD_RX} ${HEAD_RY} 0 0 1 ${round(32 - half)} ${surface} Z`;
}

/** A closed vessel, filled to its level. */
export const VESSEL: SymbolArt = {
  properties: ["level"],
  deciding: ["level"],
  states: levelStates(),
  sample: "half",
  read: readNumbers("level"),
  rules: [{ part: "liquid", level: "level" }],
  outline: VESSEL_OUTLINE,
  draw: (view) => {
    const level = view.parts?.liquid?.level;
    return [
      el("path", { "data-part": "nozzles", d: `M32 0 V${VESSEL_TOP} M32 ${VESSEL_BOTTOM} V64`, fill: "none", ...LINE }),
      el("path", { "data-part": "body", d: VESSEL_OUTLINE, fill: view.unknown ? HATCH : BODY, stroke: "none" }),
      ...(view.unknown || level === undefined || level <= 0
        ? []
        : [el("path", { "data-part": "liquid", "data-level": round(level), d: vesselLiquid(level), fill: LIQUID })]),
      el("path", {
        "data-part": "heads",
        d: `M${VESSEL_LEFT} 14 H${VESSEL_RIGHT} M${VESSEL_LEFT} 50 H${VESSEL_RIGHT}`,
        fill: "none",
        ...LINE,
        "stroke-width": 1,
        "stroke-dasharray": "3 3",
      }),
      el("path", { "data-part": "vessel", d: VESSEL_OUTLINE, fill: "none", ...LINE }),
    ];
  },
};

/** A heat exchanger: the shell the product passes, and the medium's tube coil through it. */
export const HEAT_EXCHANGER: SymbolArt = {
  properties: ["temperature", "fault"],
  deciding: ["fault", "temperature"],
  sample: "working",
  states: [
    { id: "working", name: "Working", values: { temperature: { share: 0.6 }, fault: false }, needs: ["temperature"] },
    { id: "fault", name: "Fault", values: { temperature: { share: 0.6 }, fault: true }, needs: ["fault"] },
  ],
  read: readNumbers("temperature"),
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M0 32 H10 M54 32 H64", fill: "none", ...LINE }),
    el("circle", { "data-part": "shell", cx: 32, cy: 32, r: 22, fill: view.unknown ? HATCH : BODY, ...LINE }),
    el("path", {
      "data-part": "coil",
      d: "M32 0 V14 L22 20 L42 28 L22 36 L42 44 L32 50 V64",
      fill: "none",
      ...LINE,
      "stroke-linejoin": "round",
    }),
  ],
};

/** A filter: the housing with its filter medium across it; clogged, the medium shows it. */
export const FILTER: SymbolArt = {
  properties: ["differential-pressure", "clogged", "fault"],
  deciding: ["fault", "clogged", "differential-pressure"],
  sample: "clean",
  states: [
    {
      id: "clean",
      name: "Clean",
      values: { "differential-pressure": { share: 0.2 }, clogged: false, fault: false },
      needs: ["differential-pressure|clogged"],
    },
    {
      id: "clogged",
      name: "Clogged",
      values: { "differential-pressure": { share: 0.9 }, clogged: true, fault: false },
      needs: ["clogged"],
    },
    {
      id: "fault",
      name: "Fault",
      values: { "differential-pressure": { share: 0.2 }, clogged: false, fault: true },
      needs: ["fault"],
    },
  ],
  read: (value, frame, faulted, object) => {
    const numbers = readNumbers("differential-pressure")(value, frame, faulted, object);
    if (faulted || !on(value("clogged"))) return numbers;
    return { state: "clogged", word: say("Clogged"), fault: false, view: { unknown: false, form: "clogged" } };
  },
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M0 32 H6 M58 32 H64", fill: "none", ...LINE }),
    el("path", {
      "data-part": "housing",
      d: "M6 32 L32 8 L58 32 L32 56 Z",
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
      "stroke-linejoin": "round",
    }),
    el("path", {
      "data-part": "medium",
      d: "M32 10 V54",
      fill: "none",
      ...LINE,
      stroke: view.form === "clogged" ? token("warning") : LINE.stroke,
      "stroke-width": view.form === "clogged" ? 3 : 2,
      "stroke-dasharray": "4 3",
    }),
  ],
};

/** A cyclone: its feed enters the cylinder at the side, gas leaves on top, solids down the cone. */
export const CYCLONE: SymbolArt = {
  properties: ["differential-pressure", "fault"],
  deciding: ["fault", "differential-pressure"],
  sample: "working",
  states: [
    {
      id: "working",
      name: "Working",
      values: { "differential-pressure": { share: 0.5 }, fault: false },
      needs: ["differential-pressure"],
    },
    { id: "fault", name: "Fault", values: { "differential-pressure": { share: 0.5 }, fault: true }, needs: ["fault"] },
  ],
  read: readNumbers("differential-pressure"),
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M0 14 H18 M32 0 V6 M32 58 V64", fill: "none", ...LINE }),
    el("path", {
      "data-part": "body",
      d: "M18 6 H46 V26 L35 58 H29 L18 26 Z",
      fill: view.unknown ? HATCH : BODY,
      stroke: "none",
    }),
    el("path", { "data-part": "cylinder", d: "M18 26 V6 H46 V26", fill: "none", ...LINE, "stroke-linejoin": "round" }),
    el("path", {
      "data-part": "cone",
      d: "M18 26 L29 58 H35 L46 26 Z",
      fill: "none",
      ...LINE,
      "stroke-linejoin": "round",
    }),
    // The vortex finder: the gas outlet's tube reaching into the cylinder.
    el("path", { "data-part": "vortex-finder", d: "M28 6 V20 M36 6 V20", fill: "none", ...LINE, "stroke-width": 1.5 }),
  ],
};

/** A crusher: the housing with its breaker plate and the hammer rotor, which turns while it runs. */
export const CRUSHER: SymbolArt = {
  ...MACHINE,
  properties: [...MACHINE.properties, "load"],
  read: readRunningWith("load", true),
  states: runningStates((running) => ({ load: running ? 72 : 0 })),
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M32 0 V12 M32 58 V64", fill: "none", ...LINE }),
    el("path", {
      "data-part": "housing",
      d: "M10 12 H54 V48 L44 58 H20 L10 48 Z",
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
      "stroke-linejoin": "round",
    }),
    el("path", {
      "data-part": "breaker-plate",
      d: "M54 20 L48 24 L54 28 L48 32 L54 36 L48 40",
      fill: "none",
      ...LINE,
      "stroke-linejoin": "round",
    }),
    ...(view.unknown
      ? []
      : [
          rotor(30, 34, 15, view, [
            ...[0, 90, 180, 270].map((angle) =>
              el("rect", {
                x: 28,
                y: 20,
                width: 4,
                height: 9,
                rx: 1,
                fill: paint(view),
                transform: `rotate(${angle} 30 34)`,
              }),
            ),
            el("circle", { cx: 30, cy: 34, r: 6, fill: BODY, ...LINE }),
          ]),
        ]),
  ],
};

/** A vibrating screen: the inclined deck with its mesh, on springs, and its exciter on top, whose
 * weight circles while it runs; oversize leaves at the deck's low end, undersize below it. */
export const SCREEN: SymbolArt = {
  ...MACHINE,
  read: readRunning,
  states: runningStates(),
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M8 0 V21 M58 44 H64 M32 47 V64", fill: "none", ...LINE }),
    el("path", {
      "data-part": "deck",
      d: "M6 20 L58 40 V48 L6 28 Z",
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
      "stroke-linejoin": "round",
    }),
    el("path", {
      "data-part": "mesh",
      d: "M9 25 L55 43",
      fill: "none",
      ...LINE,
      "stroke-width": 1.5,
      "stroke-dasharray": "3 3",
    }),
    el("path", {
      "data-part": "springs",
      d: "M14 31 L11 34 L17 37 L11 40 L14 42 M50 45 L47 48 L53 51 L47 54 L50 56",
      fill: "none",
      ...LINE,
      "stroke-width": 1.5,
      "stroke-linejoin": "round",
    }),
    el("path", { "data-part": "mount", d: "M32 22 V30", fill: "none", ...LINE }),
    el("circle", { "data-part": "exciter", cx: 32, cy: 14, r: 8, fill: BODY, ...LINE }),
    ...(view.unknown ? [] : [rotor(32, 14, 6, view, [el("circle", { cx: 32, cy: 9.5, r: 2.5, fill: paint(view) })])]),
  ],
};
