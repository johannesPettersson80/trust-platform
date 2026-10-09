// The supply line: where a utility enters the drawing, at a fixed or a regulated pressure. The
// connector points the way the supply flows; a regulated supply has its pressure regulator, a
// valve with a diaphragm sensing the pressure downstream (ISA-5.1) (specification 39, §5.2
// phase 2, liquid).

import { el } from "../svg";
import type { SymbolDefinition } from "./index";
import { readNumbers } from "./reading";
import { BODY, HATCH, LINE } from "./style";

export const SUPPLY: SymbolDefinition = {
  size: [64, 64],
  ports: {
    outlet: { at: [64, 32], direction: "right" },
  },
  properties: ["pressure"],
  deciding: ["pressure"],
  states: [
    { id: "normal", name: "Normal", values: { pressure: { share: 0.6 } }, needs: ["pressure"] },
    { id: "low", name: "Low", values: { pressure: { share: 0.1 } }, needs: ["pressure"] },
  ],
  sample: "normal",
  read: readNumbers("pressure"),
  draw: (view) => [
    el("path", {
      "data-part": "connector",
      d: "M4 22 H22 L32 32 L22 42 H4 Z",
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    ...(view.variant === "regulated"
      ? [
          el("g", { "data-part": "regulator" }, [
            el("path", { d: "M33 32 H38 M58 32 H64 M48 32 V20 M55 20 H61 V31", fill: "none", ...LINE }),
            el("path", { d: "M38 25 L48 32 L38 39 Z M58 25 L48 32 L58 39 Z", fill: BODY, ...LINE }),
            el("path", { d: "M41 20 A7 6 0 0 1 55 20 Z", fill: BODY, ...LINE }),
          ]),
        ]
      : [el("path", { "data-part": "line", d: "M33 32 H64", fill: "none", ...LINE })]),
  ],
};
