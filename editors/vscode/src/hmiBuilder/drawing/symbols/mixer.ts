// The mixing point: a static mixer (ISO 10628-2), where a second stream joins from below and the
// mixer's elements blend it into the main stream (specification 39, §5.2 phase 2, liquid).

import { el } from "../svg";
import type { SymbolArt } from "./index";
import { readNumbers } from "./reading";
import { BODY, HATCH, LINE } from "./style";

export const MIXER: SymbolArt = {
  properties: ["ratio"],
  deciding: ["ratio"],
  states: [{ id: "mixing", name: "Mixing", values: { ratio: { share: 0.5 } }, needs: ["ratio"] }],
  sample: "mixing",
  read: readNumbers("ratio"),
  draw: (view) => [
    el("path", { "data-part": "pipes", d: "M0 32 H13 M51 32 H64 M24 43 V64", fill: "none", ...LINE }),
    el("rect", {
      "data-part": "body",
      x: 14,
      y: 22,
      width: 36,
      height: 20,
      rx: 4,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    el("path", {
      "data-part": "elements",
      d: "M18 32 L22 26 L27 38 L32 26 L37 38 L42 26 L46 32",
      fill: "none",
      ...LINE,
    }),
  ],
};
