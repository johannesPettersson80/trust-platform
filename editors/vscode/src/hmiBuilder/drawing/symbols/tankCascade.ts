// The tank cascade: an upper open tank that drains into a lower one, each filled to its own level
// (specification 39, §5.2 phase 2, liquid; §5.3 item 5).

import { el } from "../svg";
import type { SymbolArt, SymbolState } from "./index";
import { readNumbers } from "./reading";
import { LINE } from "./style";
import { openVessel, vesselOutline, Walls } from "./vessels";

/** The two tanks' walls, in the symbol's own pixels. */
const UPPER: Walls = { left: 6, right: 42, top: 8, bottom: 30 };
const LOWER: Walls = { left: 54, right: 90, top: 40, bottom: 66 };

function state(id: string, name: string, upper: number, lower: number): SymbolState {
  return {
    id,
    name,
    values: { "upper-level": { share: upper }, "lower-level": { share: lower } },
    needs: ["upper-level|lower-level"],
  };
}

export const TANK_CASCADE: SymbolArt = {
  properties: ["upper-level", "lower-level"],
  deciding: ["upper-level", "lower-level"],
  states: [
    state("upper-full", "Upper full", 0.8, 0.3),
    state("even", "Even", 0.5, 0.5),
    state("lower-full", "Lower full", 0.2, 0.85),
  ],
  sample: "even",
  read: readNumbers("upper-level", "lower-level"),
  rules: [
    { part: "upper-liquid", level: "upper-level" },
    { part: "lower-liquid", level: "lower-level" },
  ],
  outline: `${vesselOutline(UPPER)} ${vesselOutline(LOWER)}`,
  draw: (view) => [
    ...openVessel(UPPER, "upper-liquid", view.parts?.["upper-liquid"]?.level, view.unknown),
    ...openVessel(LOWER, "lower-liquid", view.parts?.["lower-liquid"]?.level, view.unknown),
    el("path", { "data-part": "inlet", d: "M14 0 V12", fill: "none", ...LINE }),
    // The upper tank drains from its bottom into the lower one, which it enters from above.
    el("path", { "data-part": "drain", d: "M24 31 V35 H66 V46", fill: "none", ...LINE }),
    el("path", { "data-part": "outlet", d: "M91 60 H96", fill: "none", ...LINE }),
  ],
};
