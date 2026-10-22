// The instruments (ISA-5.1): a field-mounted instrument is a bubble with the letters of what it
// measures and does (TT: temperature transmitter), its tag and value beside it, so the line to
// what it measures can leave it freely (specification 39, §5.2 phase 2, instrumentation; §6
// item 5). The redundant sensor pair is two bubbles, A and B, with a mark between them that shows
// whether they agree.

import { token } from "../parts";
import { say } from "../speech";
import { el, SvgNode, text } from "../svg";
import { withValueFormat } from "../valueFormats";
import type { SymbolArt } from "./index";
import { on, readNumbers, shownBoundNumber } from "./reading";
import { BODY, HATCH, LINE } from "./style";

/** One bubble with its letters, centred at `x`. */
function bubble(x: number, letters: string, unknown: boolean, part = "bubble"): SvgNode[] {
  return [
    el("circle", { "data-part": part, cx: x, cy: 24, r: 20, fill: unknown ? HATCH : BODY, ...LINE }),
    text(letters, {
      "data-part": "letters",
      x,
      y: 29,
      fill: token("text"),
      "font-size": 14,
      "font-weight": 600,
      "text-anchor": "middle",
    }),
  ];
}

/** An instrument measuring one value, with the ISA letters of what it measures. */
export function instrument(letters: string): SymbolArt {
  return {
    properties: ["value", "fault"],
    deciding: ["fault", "value"],
    states: [
      { id: "measuring", name: "Measuring", values: { value: { share: 0.6 }, fault: false }, needs: ["value"] },
      { id: "fault", name: "Fault", values: { value: { share: 0.6 }, fault: true }, needs: ["fault"] },
    ],
    sample: "measuring",
    read: readNumbers("value"),
    draw: (view) => bubble(24, letters, view.unknown),
  };
}

const readPair = readNumbers("a", "b");

export const SENSOR_PAIR: SymbolArt = {
  properties: ["a", "b", "disagree"],
  deciding: ["a", "b", "disagree"],
  states: [
    { id: "agree", name: "Agree", values: { a: { share: 0.6 }, b: { share: 0.61 }, disagree: false }, needs: ["a|b"] },
    {
      id: "disagree",
      name: "Disagree",
      values: { a: { share: 0.6 }, b: { share: 0.75 }, disagree: true },
      needs: ["disagree"],
    },
  ],
  sample: "agree",
  // A disagreement is a fault of the measurement: its word, in the alarm colour, with its sign.
  read: (value, frame, faulted, object) =>
    on(value("disagree"))
      ? { state: "fault", word: say("Disagree"), fault: true, view: { unknown: false } }
      : readPair(value, frame, faulted, object),
  rules: [{ part: "disagreement", alarm: "disagree" }],
  draw: (view) => {
    const disagree = view.parts?.disagreement?.alarm === true;
    // Between the bubbles: an equals sign while they agree, crossed through when they do not.
    const mark = disagree ? "M50 21 H62 M50 27 H62 M53 32 L59 16" : "M50 21 H62 M50 27 H62";
    return [
      ...bubble(24, "A", view.unknown, "bubble"),
      ...bubble(88, "B", view.unknown, "bubble-b"),
      el("path", {
        "data-part": disagree ? "disagreement" : "agreement",
        d: mark,
        fill: "none",
        ...LINE,
        stroke: disagree ? token("alarm") : token("muted"),
        "stroke-width": disagree ? 2.5 : 2,
      }),
    ];
  },
};

/** A controller's letters and loop number from its tag: TIC and 101 for TIC-101; IC without a
 * tag. */
function loopOf(tag: string | undefined): { letters: string; number: string } {
  const match = /^([A-Za-z]+)[\s_-]*([0-9][0-9A-Za-z]*)?/.exec(tag ?? "");
  return { letters: match?.[1]?.toUpperCase() ?? "IC", number: match?.[2] ?? "" };
}

const readMeasurement = readNumbers("pv");

/** The loop controller of the PID loop and controller pairs (§8.1): an ISA-5.1 shared display, a
 * bubble in a square with a line across it, the letters above the line and the loop number below
 * it. Its word gives the measurement, and the setpoint beside it while they differ. */
export const CONTROLLER: SymbolArt = {
  properties: ["pv", "sp", "manual"],
  deciding: ["pv", "sp"],
  states: [
    {
      id: "controlling",
      name: "Controlling",
      values: { pv: { share: 0.58 }, sp: { share: 0.62 }, manual: false },
      needs: ["pv"],
    },
    {
      id: "on-setpoint",
      name: "On setpoint",
      values: { pv: { share: 0.62 }, sp: { share: 0.62 }, manual: false },
      needs: ["pv"],
    },
    {
      id: "manual",
      name: "Manual",
      values: { pv: { share: 0.58 }, sp: { share: 0.62 }, manual: true },
      needs: ["manual"],
    },
  ],
  sample: "controlling",
  read: (value, frame, faulted, object) => {
    const reading = readMeasurement(value, frame, faulted, object);
    const [pv, sp] = [shownBoundNumber(value, "pv", frame, object), shownBoundNumber(value, "sp", frame, object)];
    if (faulted || !pv || !sp || pv === sp) return reading;
    // One unit for the two: `42.2 → 45.0 °C`.
    const unit = withValueFormat(object, frame, "sp").unit;
    const measured = unit && pv.endsWith(` ${unit}`) && sp.endsWith(` ${unit}`) ? pv.slice(0, -unit.length - 1) : pv;
    return { ...reading, word: `${measured} → ${sp}` };
  },
  draw: (view) => {
    const { letters, number } = loopOf(view.tag);
    return [
      el("rect", { "data-part": "frame", x: 2, y: 2, width: 44, height: 44, rx: 3, fill: "none", ...LINE }),
      el("circle", { "data-part": "bubble", cx: 24, cy: 24, r: 20, fill: view.unknown ? HATCH : BODY, ...LINE }),
      el("path", { d: "M4 24 H44", fill: "none", ...LINE }),
      text(letters, {
        "data-part": "letters",
        x: 24,
        y: 19.5,
        fill: token("text"),
        "font-size": letters.length > 3 ? 9 : 11,
        "font-weight": 650,
        "text-anchor": "middle",
      }),
      ...(number
        ? [
            text(number, {
              "data-part": "loop",
              x: 24,
              y: 37,
              fill: token("muted"),
              "font-size": number.length > 3 ? 8.5 : 10,
              "font-weight": 600,
              "text-anchor": "middle",
            }),
          ]
        : []),
    ];
  },
};

/** The analyzer (AT): a measured quality such as pH, conductivity or turbidity. */
export const ANALYZER = instrument("AT");

/** A switch instrument: its bubble with its letters, its state as On or Off. */
function switchInstrument(letters: string): SymbolArt {
  return {
    properties: ["state", "fault"],
    deciding: ["fault", "state"],
    states: [
      { id: "off", name: "Off", values: { state: false, fault: false }, needs: ["state"] },
      { id: "on", name: "On", values: { state: true, fault: false }, needs: ["state"] },
      { id: "fault", name: "Fault", values: { state: false, fault: true }, needs: ["fault"] },
    ],
    sample: "off",
    read: (value, _frame, faulted) => {
      const state = value("state");
      return {
        state: faulted ? "fault" : state ? (on(state) ? "on" : "off") : undefined,
        word: faulted ? say("Fault") : state ? say(on(state) ? "On" : "Off") : "",
        fault: faulted,
        view: { unknown: false },
      };
    },
    draw: (view) => bubble(24, letters, view.unknown),
  };
}

export const LEVEL_SWITCH = switchInstrument("LS");
export const PRESSURE_SWITCH = switchInstrument("PS");
/** Any switch, named by its tag's letters: FS for a flow switch FS-101, XS without a tag. */
export const DIGITAL_SWITCH: SymbolArt = {
  ...switchInstrument("XS"),
  draw: (view) => bubble(24, /^([A-Za-z]{1,4})/.exec(view.tag ?? "")?.[1]?.toUpperCase() ?? "XS", view.unknown),
};
