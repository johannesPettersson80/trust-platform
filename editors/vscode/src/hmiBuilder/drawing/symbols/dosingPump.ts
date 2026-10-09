// The dosing pump: a reciprocating metering pump (ISO 10628-2). Its piston strokes to and fro
// while it runs, drawing from below and delivering above; its drive sits beside the cylinder
// (specification 39, §5.2 phase 2, liquid; §5.3 items 4 and 5).

import { token } from "../parts";
import { el } from "../svg";
import type { SymbolDefinition } from "./index";
import { readRunningWith, runningStates } from "./reading";
import { BODY, HATCH, LINE, MOVING, STILL } from "./style";

export const DOSING_PUMP: SymbolDefinition = {
  size: [64, 64],
  ports: {
    inlet: { at: [14, 64], direction: "down" },
    outlet: { at: [14, 0], direction: "up" },
  },
  properties: ["running", "fault", "manual", "out-of-service", "rate"],
  deciding: ["fault", "running"],
  states: runningStates((running) => ({ rate: running ? { share: 0.6 } : 0 })),
  sample: "running",
  read: readRunningWith("rate", false),
  rules: [{ part: "piston", stroke: "running" }],
  draw: (view) => {
    const stroke = view.parts?.piston?.stroke;
    const paint = stroke ? MOVING : STILL;
    return [
      el("path", { "data-part": "suction", d: "M14 45 V64", fill: "none", ...LINE }),
      el("path", { "data-part": "discharge", d: "M14 0 V21", fill: "none", ...LINE }),
      // The way the liquid goes: in from below, out above.
      el("path", {
        "data-part": "flow",
        d: "M10.5 58 L14 53 L17.5 58 Z M10.5 11 L14 6 L17.5 11 Z",
        fill: token("muted"),
      }),
      el("rect", {
        "data-part": "cylinder",
        x: 6,
        y: 22,
        width: 34,
        height: 22,
        rx: 4,
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
      }),
      ...(view.unknown
        ? []
        : [
            el(
              "g",
              {
                "data-part": "piston",
                class: stroke ? "hmi-stroke" : undefined,
                style: stroke ? `animation-delay: -${stroke.delayMs}ms` : undefined,
              },
              [
                el("rect", { x: 22, y: 25, width: 4, height: 16, rx: 1, fill: paint }),
                el("path", {
                  d: "M26 33 H48",
                  fill: "none",
                  stroke: paint,
                  "stroke-width": 2.5,
                  "stroke-linecap": "round",
                  "vector-effect": "non-scaling-stroke",
                }),
              ],
            ),
          ]),
      el("rect", { "data-part": "drive", x: 46, y: 25, width: 14, height: 16, rx: 3, fill: BODY, ...LINE }),
    ];
  },
};
