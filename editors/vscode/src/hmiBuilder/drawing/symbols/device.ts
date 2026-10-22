// The plain device (specification 39, §21.9): the picture of a customer's own function block until
// they choose one of their own in My Symbols. A rounded unit with a control panel's lines, its lamp
// lit in the moving colour while it runs.

import { el } from "../svg";
import type { SymbolArt } from "./index";
import { readRunning, runningStates } from "./reading";
import { BODY, HATCH, LINE, MOVING, STILL } from "./style";

export const DEVICE: SymbolArt = {
  properties: ["running", "fault", "manual", "out-of-service"],
  deciding: ["fault", "running"],
  states: runningStates(),
  sample: "running",
  read: readRunning,
  rules: [{ part: "lamp", active: "running" }],
  draw: (view) => [
    el("rect", {
      "data-part": "body",
      x: 8,
      y: 12,
      width: 48,
      height: 40,
      rx: 8,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    el("path", {
      "data-part": "panel",
      d: "M18 26 H36 M18 33 H32 M18 40 H36",
      fill: "none",
      ...LINE,
      "stroke-linecap": "round",
    }),
    el("circle", {
      "data-part": "lamp",
      cx: 45,
      cy: 23,
      r: 4,
      fill: view.unknown ? "none" : view.parts?.lamp?.active ? MOVING : STILL,
      ...LINE,
    }),
  ],
};
