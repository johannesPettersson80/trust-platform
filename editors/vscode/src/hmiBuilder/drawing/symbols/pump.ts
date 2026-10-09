// The centrifugal pump (ISO 10628-2): the casing with its tangential outlet on its stand, and the
// impeller inside, which turns while the pump runs (specification 39, §5.3 item 4, §14.2). The
// variable-speed variant carries the arrow of variability and gives its speed (§5.2 phase 2).

import { el, SvgNode } from "../svg";
import type { SymbolDefinition, SymbolView } from "./index";
import { readRunningWith, runningStates } from "./reading";
import { BODY, HATCH, LINE, MOVING, round, STILL } from "./style";

const CENTRE = { x: 30, y: 36 };
const VANES = 6;

export const PUMP: SymbolDefinition = {
  size: [64, 64],
  properties: ["running", "fault", "manual", "out-of-service", "speed"],
  deciding: ["fault", "running"],
  sample: "running",
  // A variable-speed pump gives its speed while it runs.
  read: readRunningWith("speed", true),
  states: runningStates((running) => ({ speed: running ? 62 : 0 })),
  ports: {
    inlet: { at: [0, CENTRE.y], direction: "left" },
    outlet: { at: [64, 20], direction: "right" },
  },
  draw: (view) => [
    // A variable-speed pump: the ISO 14617 arrow of variability, passing behind the casing.
    ...(view.variant === "variable-speed"
      ? [
          el("path", {
            "data-part": "variable",
            d: "M3 61 L59 5 M59 5 L51.3 7.1 M59 5 L56.9 12.7",
            fill: "none",
            ...LINE,
          }),
        ]
      : []),
    el("path", { "data-part": "inlet", d: `M0 ${CENTRE.y} H10`, fill: "none", ...LINE }),
    el("path", { "data-part": "stand", d: "M21 51 L16 62 H44 L39 51 Z", fill: BODY, ...LINE }),
    // The casing and its outlet in one outline: the outlet leaves the top of the casing along its
    // tangent and ends at the symbol's right edge.
    el("path", {
      "data-part": "casing",
      d: "M30 16 H64 V24 H46 A20 20 0 1 1 30 16 Z",
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    ...(view.unknown ? [] : [impeller(view)]),
  ],
};

/** The hub and six backward-curved vanes; the group's span is the whole wheel, so it turns about the hub. */
function impeller(view: SymbolView): SvgNode {
  const paint = view.turning ? MOVING : STILL;
  const point = (radius: number, degrees: number): string => {
    const angle = (degrees * Math.PI) / 180;
    return `${round(CENTRE.x + radius * Math.cos(angle))} ${round(CENTRE.y + radius * Math.sin(angle))}`;
  };
  const vanes = Array.from({ length: VANES }, (_, index) => {
    const root = (360 / VANES) * index;
    return `M${point(5.5, root)} Q${point(11, root - 12)} ${point(14, root - 42)}`;
  }).join(" ");
  return el(
    "g",
    {
      "data-part": "impeller",
      class: view.turning ? "hmi-spin" : undefined,
      style: view.turning ? `animation-delay: -${view.turning.delayMs}ms` : undefined,
    },
    [
      el("circle", { cx: CENTRE.x, cy: CENTRE.y, r: 15, fill: "none", stroke: "none" }),
      el("path", {
        d: vanes,
        fill: "none",
        stroke: paint,
        "stroke-width": 2.5,
        "stroke-linecap": "round",
        "vector-effect": "non-scaling-stroke",
      }),
      el("circle", { cx: CENTRE.x, cy: CENTRE.y, r: 3.5, fill: paint }),
    ],
  );
}
