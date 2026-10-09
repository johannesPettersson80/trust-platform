// The control valve: a globe valve with a spring-diaphragm actuator (ISA-5.1). The plug stands at
// the valve's position, on its seat in the middle of the body when closed and lifted by its full
// travel when open (specification 39, §5.3 item 4, §14.2). With a positioner (§5.2 phase 2), its
// box sits on the stem and a pointer shows where it sends the stem: the demand.

import { formatValue } from "../format";
import { say } from "../speech";
import { el } from "../svg";
import type { SymbolDefinition, SymbolState } from "./index";
import type { LiveValue } from "../model";
import { token } from "../parts";
import { percentWord, SymbolReading } from "./reading";
import { BODY, FILLED, HATCH, LINE, MOVING, round } from "./style";
import { AXIS, BODY_PATH, MIDDLE, STUBS_PATH } from "./valve";

/** How far the plug lifts from closed to open: out of the body when open. */
const TRAVEL = 13;
/** The underside of the actuator, where the stem goes in. */
const ACTUATOR = 20;

/** A state of the control valve: its position and demand in percent, and its flags. */
function state(
  id: string,
  name: string,
  position: number,
  flags: Partial<Record<"fault" | "manual" | "out-of-service", boolean>> = {},
  demand = position,
): SymbolState {
  const set = Object.keys(flags);
  const needs = set.length > 0 ? set : demand !== position ? ["position", "demand"] : ["position"];
  return {
    id,
    name,
    values: { position, demand, fault: false, manual: false, "out-of-service": false, ...flags },
    needs,
  };
}

/** How high the plug's top stands at a lift from 0 (closed) to 1 (open). */
function plugTop(lift: number): number {
  return AXIS - 8 - TRAVEL * Math.min(1, Math.max(0, lift));
}

export const CONTROL_VALVE: SymbolDefinition = {
  size: [64, 64],
  properties: ["position", "fault", "manual", "out-of-service", "demand"],
  deciding: ["fault", "position"],
  sample: "partly-open",
  read: (value, _frame, faulted) => readPosition(value("position"), value("demand"), faulted),
  rules: [{ part: "demand", travel: "demand" }],
  states: [
    state("closed", "Closed", 0),
    state("partly-open", "60 %", 60),
    state("moving", "Moving", 40, {}, 70),
    state("open", "Open", 100),
    state("fault", "Fault", 60, { fault: true }),
    state("manual", "Manual", 60, { manual: true }),
    state("out-of-service", "Out of service", 0, { "out-of-service": true }),
  ],
  ports: {
    inlet: { at: [0, AXIS], direction: "left" },
    outlet: { at: [64, AXIS], direction: "right" },
  },
  draw: (view) => {
    const lift = Math.min(1, Math.max(0, view.opening ?? 0));
    const top = plugTop(lift);
    const demand = view.unknown ? undefined : view.parts?.demand?.travel;
    return [
      // The positioner first, so the stem and the plug lie over its link.
      ...(view.variant === "positioner"
        ? [
            el("g", { "data-part": "positioner" }, [
              el("path", { d: `M${MIDDLE} 27 H42`, fill: "none", ...LINE }),
              el("rect", { x: 42, y: 22, width: 14, height: 10, rx: 2, fill: BODY, ...LINE }),
            ]),
          ]
        : []),
      el("path", { d: STUBS_PATH, fill: "none", ...LINE }),
      el("path", {
        "data-part": "body",
        "data-filled": String(view.filled === true),
        d: BODY_PATH,
        fill: view.unknown ? HATCH : view.filled ? FILLED : BODY,
        ...LINE,
      }),
      el("path", {
        "data-part": "stem",
        d: `M${MIDDLE} ${view.unknown ? AXIS - 8 : round(top)} V${ACTUATOR}`,
        fill: "none",
        ...LINE,
      }),
      el("path", { "data-part": "actuator", d: `M15 ${ACTUATOR} A17 12 0 0 1 49 ${ACTUATOR} Z`, fill: BODY, ...LINE }),
      ...(view.unknown
        ? []
        : [
            el("path", {
              "data-part": "plug",
              "data-lift": round(lift),
              d: `M${MIDDLE - 4} ${round(top)} H${MIDDLE + 4} V${round(top + 3)} Q${MIDDLE + 4} ${round(top + 8)} ${MIDDLE} ${round(top + 8)} Q${MIDDLE - 4} ${round(top + 8)} ${MIDDLE - 4} ${round(top + 3)} Z`,
              fill: MOVING,
            }),
          ]),
      ...(demand !== undefined
        ? [
            el("path", {
              "data-part": "demand",
              "data-travel": round(demand),
              d: `M22 ${round(plugTop(demand) - 3)} L26.5 ${round(plugTop(demand))} L22 ${round(plugTop(demand) + 3)} Z`,
              fill: token("muted"),
            }),
          ]
        : []),
    ];
  },
};

/** The plug stands at the position, 0 closed to 100 open; the word gives the position, and the
 * demand beside it while the two differ. */
function readPosition(position: LiveValue | undefined, demand: LiveValue | undefined, faulted: boolean): SymbolReading {
  const travel = Number(position?.value);
  if (position === undefined || !Number.isFinite(travel)) {
    return {
      state: faulted ? "fault" : undefined,
      word: faulted ? say("Fault") : "",
      fault: faulted,
      view: { unknown: false },
    };
  }
  const percent = Math.min(100, Math.max(0, travel));
  const at = say("{value} %", { value: formatValue(Math.round(percent), {}) });
  const wanted = Number(demand?.value);
  const sent =
    demand !== undefined &&
    demand.value !== null &&
    Number.isFinite(wanted) &&
    Math.round(wanted) !== Math.round(percent)
      ? percentWord(demand)
      : undefined;
  return {
    state: faulted ? "fault" : percent <= 0 ? "closed" : "open",
    word: faulted ? say("Fault") : sent ? `${at} → ${sent}` : at,
    fault: faulted,
    view: { unknown: false, opening: percent / 100, filled: percent <= 0 },
  };
}
