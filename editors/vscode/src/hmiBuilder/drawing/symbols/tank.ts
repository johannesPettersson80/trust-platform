// The open tank (ISO 10628-2): open at the top, filled from above through its inlet pipe, drawn
// off through the outlet near its bottom, and filled to its level (specification 39, §5.2 phase
// 2, liquid; §5.3 item 5).

import { el } from "../svg";
import type { SymbolArt, SymbolState } from "./index";
import { readNumbers } from "./reading";
import { LINE } from "./style";
import { openVessel, vesselOutline, Walls } from "./vessels";

/** The tank's walls, in its own pixels. */
const WALLS: Walls = { left: 8, right: 56, top: 8, bottom: 54 };

/** A tank's states: its level as a share of the object's range. */
export function levelStates(property = "level"): SymbolState[] {
  const state = (id: string, name: string, share: number): SymbolState => ({
    id,
    name,
    values: { [property]: { share } },
    needs: [property],
  });
  return [state("low", "Low", 0.15), state("half", "Half", 0.5), state("high", "High", 0.9)];
}

export const TANK: SymbolArt = {
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
    el("path", { "data-part": "outlet", d: "M57 46 H64", fill: "none", ...LINE }),
  ],
};
