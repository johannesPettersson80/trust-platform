// The electrical equipment of phase 3 (specification 39, §5.2), drawn as IEC 60617 draws it: the
// frequency converter and the drive, a square with its diagonal and what goes in and out; the
// contactor's main contact with its arc, and the circuit breaker's with its cross. They have no
// ports: a single-line diagram joins them with lines.

import { say } from "../speech";
import { el, SvgNode } from "../svg";
import type { SymbolDefinition, SymbolState, SymbolView } from "./index";
import { paint } from "./plant";
import { on, readRunningWith, runningStates } from "./reading";
import type { SymbolReader } from "./reading";
import { BODY, HATCH, LINE, MOVING, STILL } from "./style";

const WAVE = "q3 -4 6 0 t6 0";

/** The converter: its terminals, the square with its diagonal, the input's sign at the upper left
 * and the output's at the lower right, drawn in the moving colour while it runs. */
function converter(view: SymbolView, input: SvgNode): SvgNode[] {
  return [
    el("path", { "data-part": "terminals", d: "M32 0 V12 M32 52 V64", fill: "none", ...LINE }),
    el("rect", {
      "data-part": "converter",
      x: 12,
      y: 12,
      width: 40,
      height: 40,
      rx: 2,
      fill: view.unknown ? HATCH : BODY,
      ...LINE,
    }),
    el("path", { d: "M12 52 L52 12", fill: "none", ...LINE, "stroke-width": 1.5 }),
    input,
    el("path", {
      "data-part": "output",
      d: `M34 41 ${WAVE}`,
      fill: "none",
      ...LINE,
      stroke: paint(view),
      "stroke-linecap": "round",
    }),
  ];
}

/** A frequency converter: AC in, AC out at the frequency it gives while it runs. */
export const VFD: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["running", "fault", "manual", "out-of-service", "frequency"],
  deciding: ["fault", "running"],
  sample: "running",
  states: runningStates((running) => ({ frequency: running ? { share: 0.8 } : 0 })),
  read: readRunningWith("frequency", false),
  draw: (view) =>
    converter(
      view,
      el("path", { "data-part": "input", d: `M18 23 ${WAVE}`, fill: "none", ...LINE, "stroke-linecap": "round" }),
    ),
};

const runningDrive = readRunningWith("speed", false);

/** A drive's word: Fault, Running with its speed, then Ready or Not ready while it stands. */
const readDrive: SymbolReader = (value, frame, faulted, object) => {
  const reading = runningDrive(value, frame, faulted, object);
  const ready = value("ready");
  if (faulted || reading.state === "running" || ready === undefined) return reading;
  return on(ready)
    ? { ...reading, state: "ready", word: say("Ready") }
    : { ...reading, state: "not-ready", word: say("Not ready") };
};

/** A drive: DC in from its bus, AC out to its motor at the speed it gives while it runs. */
export const DRIVE: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["running", "ready", "fault", "manual", "out-of-service", "speed"],
  deciding: ["fault", "running", "ready"],
  sample: "running",
  states: [
    ...runningStates((running) => ({ ready: true, speed: running ? { share: 0.6 } : 0 })),
    {
      id: "not-ready",
      name: "Not ready",
      values: { running: false, ready: false, fault: false, manual: false, "out-of-service": false, speed: 0 },
      needs: ["ready"],
    } satisfies SymbolState,
  ],
  read: readDrive,
  draw: (view) =>
    converter(
      view,
      el("path", {
        "data-part": "input",
        d: "M18 21 H30 M18 25 H30",
        fill: "none",
        ...LINE,
        "stroke-linecap": "round",
      }),
    ),
};

/** A main contact: the fixed contact from the top terminal, the blade from its pivot below, closed
 * along the line or open at an angle, and the mark at the fixed contact that says what it is. */
function contact(view: SymbolView, closed: boolean, mark: SvgNode): SvgNode[] {
  return [
    el("path", { "data-part": "terminals", d: "M32 0 V22 M32 46 V64", fill: "none", ...LINE }),
    el("circle", { cx: 32, cy: 46, r: 2, fill: view.unknown ? HATCH : BODY, ...LINE }),
    el("path", {
      "data-part": "contact",
      "data-closed": String(closed),
      d: closed ? "M32 46 L32 22" : "M32 46 L20 25",
      fill: "none",
      ...LINE,
      stroke: closed && !view.unknown ? MOVING : STILL,
      "stroke-width": 3,
      "stroke-linecap": "round",
    }),
    mark,
  ];
}

/** A switching device's word and contact: Open or Closed, or Fault first. */
function readSwitching(value: Parameters<SymbolReader>[0], faulted: boolean): ReturnType<SymbolReader> {
  const closed = value("closed");
  const shut = on(closed);
  return {
    state: faulted ? "fault" : closed ? (shut ? "closed" : "open") : undefined,
    word: faulted ? say("Fault") : closed ? say(shut ? "Closed" : "Open") : "",
    fault: faulted,
    view: { unknown: false, form: shut ? "closed" : "open" },
  };
}

const switchingStates = (extra: Record<string, boolean> = {}): SymbolState[] => [
  { id: "closed", name: "Closed", values: { closed: true, ...extra }, needs: ["closed"] },
  { id: "open", name: "Open", values: { closed: false, ...extra }, needs: ["closed"] },
];

/** A contactor: its main contact with the contactor's arc, closed while it is energised. */
export const CONTACTOR: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["closed", "fault"],
  deciding: ["fault", "closed"],
  sample: "closed",
  states: [
    ...switchingStates({ fault: false }),
    { id: "fault", name: "Fault", values: { closed: false, fault: true }, needs: ["fault"] },
  ],
  read: (value, _frame, faulted) => readSwitching(value, faulted),
  draw: (view) =>
    contact(
      view,
      view.form === "closed",
      el("path", { "data-part": "arc", d: "M32 14 A4 4 0 0 0 32 22", fill: "none", ...LINE }),
    ),
};

/** A circuit breaker: its main contact with the breaker's cross; tripped, it stands open and says
 * so as a fault. */
export const BREAKER: SymbolDefinition = {
  size: [64, 64],
  ports: {},
  properties: ["closed", "tripped"],
  deciding: ["tripped", "closed"],
  sample: "closed",
  states: [
    ...switchingStates({ tripped: false }),
    { id: "tripped", name: "Tripped", values: { closed: false, tripped: true }, needs: ["tripped"] },
  ],
  read: (value, _frame, faulted) => {
    if (!on(value("tripped"))) return readSwitching(value, faulted);
    return { state: "tripped", word: say("Tripped"), fault: true, view: { unknown: false, form: "open" } };
  },
  draw: (view) =>
    contact(
      view,
      view.form === "closed",
      el("path", { "data-part": "cross", d: "M28.5 18.5 L35.5 25.5 M35.5 18.5 L28.5 25.5", fill: "none", ...LINE }),
    ),
};
