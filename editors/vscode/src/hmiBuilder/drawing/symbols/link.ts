// The network link (specification 39, §5.2 phase 2, communication; §5.3): two stations and the
// link between them. While connected, its packets travel along it and its word gives their age;
// a lost link is broken, a fault; a controller holding its last value shows a pause; one that
// fell back to local control turns back on itself; one switched off on purpose is out of service,
// its line whole and still.

import { token } from "../parts";
import { say } from "../speech";
import { el, SvgNode } from "../svg";
import type { SymbolArt } from "./index";
import { observed, turnDelayMs } from "./motion";
import { on, shownBoundNumber } from "./reading";
import { BODY, HATCH, LINE, MOVING } from "./style";

const LINE_PATH = "M20 32 H44";

export const NETWORK_LINK: SymbolArt = {
  properties: ["connected", "age", "hold", "fallback", "out-of-service"],
  deciding: ["connected"],
  states: [
    {
      id: "connected",
      name: "Connected",
      values: { connected: true, age: 120, hold: false, fallback: false, "out-of-service": false },
      needs: ["connected"],
    },
    {
      id: "hold",
      name: "Hold",
      values: { connected: true, age: 900, hold: true, fallback: false, "out-of-service": false },
      needs: ["hold"],
    },
    {
      id: "lost",
      name: "Lost",
      values: { connected: false, age: 4000, hold: false, fallback: false, "out-of-service": false },
      needs: ["connected"],
    },
    {
      id: "fallback",
      name: "Fallback",
      values: { connected: false, age: 4000, hold: false, fallback: true, "out-of-service": false },
      needs: ["fallback"],
    },
    {
      id: "out-of-service",
      name: "Out of service",
      values: { connected: false, age: 4000, hold: false, fallback: false, "out-of-service": true },
      needs: ["out-of-service"],
    },
  ],
  sample: "connected",
  read: (value, frame, faulted, object) => {
    const connected = value("connected");
    if (connected === undefined) return { word: "", fault: faulted, view: { unknown: false } };
    if (!on(connected)) {
      if (on(value("out-of-service")))
        return { state: "out-of-service", word: say("Off"), fault: faulted, view: { unknown: false, form: "off" } };
      return on(value("fallback"))
        ? { state: "fallback", word: say("Fallback"), fault: faulted, view: { unknown: false, form: "fallback" } }
        : { state: "lost", word: say("Lost"), fault: true, view: { unknown: false, form: "lost" } };
    }
    if (on(value("hold")))
      return { state: "hold", word: say("Hold"), fault: faulted, view: { unknown: false, form: "hold" } };
    const age = shownBoundNumber(value, "age", frame, object);
    return {
      state: "connected",
      word: faulted ? say("Fault") : age ? `${say("Connected")} · ${age}` : say("Connected"),
      fault: faulted,
      view: {
        unknown: false,
        form: "connected",
        turning: observed(connected) ? { delayMs: turnDelayMs(frame.nowMs) } : undefined,
      },
    };
  },
  draw: (view) => {
    const stations = [
      el("rect", {
        "data-part": "station",
        x: 4,
        y: 22,
        width: 16,
        height: 20,
        rx: 3,
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
      }),
      el("rect", {
        "data-part": "station",
        x: 44,
        y: 22,
        width: 16,
        height: 20,
        rx: 3,
        fill: view.unknown ? HATCH : BODY,
        ...LINE,
      }),
    ];
    const form: Record<string, () => SvgNode[]> = {
      connected: () => [
        el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE }),
        ...(view.turning
          ? [
              el("path", {
                "data-part": "packets",
                class: "hmi-flow",
                style: `animation-delay: -${view.turning.delayMs % 800}ms`,
                d: LINE_PATH,
                fill: "none",
                stroke: MOVING,
                "stroke-width": 3,
                "stroke-dasharray": "2 6",
                "stroke-linecap": "round",
                "vector-effect": "non-scaling-stroke",
              }),
            ]
          : []),
      ],
      hold: () => [
        el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE }),
        el("path", {
          "data-part": "hold",
          d: "M29.5 26 V38 M34.5 26 V38",
          fill: "none",
          stroke: MOVING,
          "stroke-width": 3,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke",
        }),
      ],
      off: () => [el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE })],
      lost: () => [
        el("path", { "data-part": "link", d: "M20 32 H27 M37 32 H44", fill: "none", ...LINE }),
        el("path", {
          "data-part": "break",
          d: "M29 28 L35 36 M35 28 L29 36",
          fill: "none",
          stroke: token("alarm"),
          "stroke-width": 2.5,
          "stroke-linecap": "round",
          "vector-effect": "non-scaling-stroke",
        }),
      ],
      fallback: () => [
        el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE, "stroke-dasharray": "3 4" }),
        // The local station's control, turning back on itself.
        el("path", {
          "data-part": "fallback",
          d: "M46 20 C44 9 60 9 58 20 M58 20 L54.5 16.5 M58 20 L60.5 15.5",
          fill: "none",
          stroke: MOVING,
          "stroke-width": 2,
          "stroke-linecap": "round",
          "stroke-linejoin": "round",
          "vector-effect": "non-scaling-stroke",
        }),
      ],
    };
    return [
      ...(view.unknown || !view.form
        ? [el("path", { "data-part": "link", d: LINE_PATH, fill: "none", ...LINE })]
        : form[view.form]()),
      ...stations,
    ];
  },
};
