// The on/off valve: a ball valve (ISO 10628-2) with its actuator. The ball's bore lies in line with
// the pipe when open and across it when closed, and the body is filled when closed, as on a P&ID
// (specification 39, §5.3 item 4, §14.2).

import { say } from "../speech";
import { el } from "../svg";
import type { Word } from "../words";
import type { SymbolDefinition, SymbolState } from "./index";
import type { LiveValue } from "../model";
import { on, SymbolReading } from "./reading";
import { BODY, FILLED, HATCH, LINE, MOVING } from "./style";

/** The pipe's axis and the middle of the valve. */
export const AXIS = 44;
export const MIDDLE = 32;

/** The valve body: two triangles meeting in the middle, the bow-tie of every P&ID valve. */
export const BODY_PATH = `M6 32 L${MIDDLE} ${AXIS} L6 56 Z M58 32 L${MIDDLE} ${AXIS} L58 56 Z`;

/** The pipe stubs either side of the body. */
export const STUBS_PATH = `M0 ${AXIS} H6 M58 ${AXIS} H64`;

type Flag = "fault" | "manual" | "out-of-service";

/** A state of the valve: its feedbacks and flags. Open and closed need either feedback, moving
 * both. */
function state(
  id: string,
  name: string,
  open: boolean,
  closed: boolean,
  flags: Partial<Record<Flag, boolean>> = {},
): SymbolState {
  const set = Object.keys(flags);
  const needs = set.length > 0 ? set : open || closed ? ["open|closed"] : ["open", "closed"];
  return { id, name, values: { open, closed, fault: false, manual: false, "out-of-service": false, ...flags }, needs };
}

export const VALVE: SymbolDefinition = {
  size: [64, 64],
  properties: ["open", "closed", "fault", "manual", "out-of-service"],
  deciding: ["fault", "open", "closed"],
  sample: "open",
  read: (value, _frame, faulted) => readFeedback(value("open"), value("closed"), faulted),
  states: [
    state("open", "Open", true, false),
    state("closed", "Closed", false, true),
    state("moving", "Moving", false, false),
    state("fault", "Fault", false, true, { fault: true }),
    state("manual", "Manual", true, false, { manual: true }),
    state("out-of-service", "Out of service", false, true, { "out-of-service": true }),
  ],
  ports: {
    inlet: { at: [0, AXIS], direction: "left" },
    outlet: { at: [64, AXIS], direction: "right" },
  },
  draw: (view) => {
    const angle = 90 * (1 - (view.opening ?? 0));
    return [
      el("path", { d: STUBS_PATH, fill: "none", ...LINE }),
      el("path", { "data-part": "stem", d: `M${MIDDLE} 36 V22`, fill: "none", ...LINE }),
      el("rect", { "data-part": "actuator", x: 21, y: 10, width: 22, height: 12, rx: 3, fill: BODY, ...LINE }),
      el("path", {
        "data-part": "body",
        "data-filled": String(view.filled === true),
        d: BODY_PATH,
        fill: view.unknown ? HATCH : view.filled ? FILLED : BODY,
        ...LINE,
      }),
      el("circle", { "data-part": "ball", cx: MIDDLE, cy: AXIS, r: 8, fill: BODY, ...LINE }),
      ...(view.unknown
        ? []
        : [
            el("rect", {
              "data-part": "bore",
              "data-angle": angle,
              x: MIDDLE - 7,
              y: AXIS - 2,
              width: 14,
              height: 4,
              rx: 2,
              fill: MOVING,
              transform: angle === 0 ? undefined : `rotate(${angle} ${MIDDLE} ${AXIS})`,
            }),
          ]),
    ];
  },
};

/** The words of a two-position actuator's ends: a valve's or a damper's, or a cylinder's. */
export interface EndWords {
  open: Word;
  closed: Word;
}

/**
 * Open or closed by the feedbacks: with both, neither TRUE is moving and both TRUE a fault; with
 * one, the other end is its opposite.
 */
export function readFeedback(
  open: LiveValue | undefined,
  closed: LiveValue | undefined,
  faulted: boolean,
  ends: EndWords = { open: "Open", closed: "Closed" },
): SymbolReading {
  let stands: "open" | "closed" | "moving" | "both" | undefined;
  if (open && closed) stands = on(open) ? (on(closed) ? "both" : "open") : on(closed) ? "closed" : "moving";
  else if (open) stands = on(open) ? "open" : "closed";
  else if (closed) stands = on(closed) ? "closed" : "open";
  const fault = faulted || stands === "both";
  const words = { open: ends.open, closed: ends.closed, moving: "Moving", both: "Fault" } as const satisfies Record<
    string,
    Word
  >;
  const opening = stands === "open" ? 1 : stands === "closed" ? 0 : stands ? 0.5 : undefined;
  return {
    state: fault ? "fault" : stands,
    word: fault ? say("Fault") : stands ? say(words[stands]) : "",
    fault,
    view: { unknown: false, opening, filled: stands === "closed" },
  };
}
