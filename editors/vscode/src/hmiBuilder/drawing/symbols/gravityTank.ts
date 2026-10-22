// The gravity-drained tank: an open tank that empties through an orifice in its bottom, so its
// outflow grows with its level (specification 39, §5.2 phase 2, liquid; §5.3 item 5).

import { el } from "../svg";
import type { SymbolArt } from "./index";
import { readNumbers } from "./reading";
import { LINE } from "./style";
import { levelStates } from "./tank";
import { openVessel, vesselOutline, Walls } from "./vessels";

/** The tank's walls, in its own pixels. */
const WALLS: Walls = { left: 8, right: 56, top: 8, bottom: 52 };

export const GRAVITY_TANK: SymbolArt = {
  properties: ["level"],
  deciding: ["level"],
  states: levelStates(),
  sample: "half",
  read: readNumbers("level"),
  rules: [{ part: "liquid", level: "level" }],
  outline: vesselOutline(WALLS),
  draw: (view) => [
    ...openVessel(WALLS, "liquid", view.parts?.liquid?.level, view.unknown),
    el("path", { "data-part": "inlet", d: "M20 0 V14", fill: "none", ...LINE }),
    // The bottom outlet and its orifice: two short bars across the pipe.
    el("path", { "data-part": "outlet", d: "M32 53 V64 M28.5 57.5 H35.5 M28.5 60 H35.5", fill: "none", ...LINE }),
  ],
};
