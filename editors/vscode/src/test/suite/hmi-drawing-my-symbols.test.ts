import * as assert from "assert";

import { drawable } from "../../hmiBuilder/drawing/components";
import type { HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { portPoint } from "../../hmiBuilder/drawing/pipes/ports";
import { renderToString } from "../../hmiBuilder/drawing/svg";
import { registerSymbols } from "../../hmiBuilder/drawing/symbols";
import type { CustomSymbolModel } from "../../hmiBuilder/drawing/symbols/custom";
import { drawObject, part, stateWord, valuesOf, words } from "./symbolTestKit";

// Specification 39, §14.4: a project's own symbol, as the language server and the runtime give
// it, drawn as equipment is: its drawing scaled above the tag and state word, its parts driven by
// the bindings, its ports reached by pipes.

export const AGITATOR: CustomSymbolModel = {
  name: "agitator",
  title: "Agitator",
  size: [128, 108],
  viewBox: [0, 0, 64, 64],
  properties: { running: "boolean", level: "number", speed: "number", fault: "boolean" },
  ports: [{ name: "inlet", at: [0, 32], direction: "left" }],
  parts: [
    { part: "rotor", spin: "running" },
    { part: "liquid", level: "level", range: [0, 100] },
    { part: "reading", show: "speed" },
  ],
  svg: {
    tag: "g",
    attrs: {},
    children: [
      {
        tag: "rect",
        attrs: { id: "my-agitator-tank", x: "8", y: "8", width: "48", height: "48", fill: "#334" },
        children: [],
      },
      {
        tag: "rect",
        attrs: {
          id: "my-agitator-liquid",
          "data-part": "liquid",
          x: "10",
          y: "30",
          width: "44",
          height: "24",
          fill: "#3cf",
        },
        children: [],
      },
      {
        tag: "g",
        attrs: { id: "my-agitator-rotor", "data-part": "rotor" },
        children: [{ tag: "path", attrs: { d: "M32 20 L36 32 L32 44 L28 32 Z" }, children: [] }],
      },
      {
        tag: "text",
        attrs: { id: "my-agitator-reading", "data-part": "reading", x: "32", y: "60" },
        children: [],
        text: "0",
      },
    ],
  },
};

function mixer(bind: string[]): HmiObjectModel {
  return {
    id: "mixer",
    type: "my:agitator",
    tag: "M-1",
    at: [0, 0],
    size: [128, 108],
    rotation: 0,
    bind: Object.fromEntries(bind.map((name) => [name, { path: `X.${name}`, direction: "read" as const }])),
  };
}

/** The agitator's parts driven the other ways: the rotor strokes, the liquid travels upwards, the
 * reading shows only in state 2 (§5.3 item 5, §14.4 item 2). */
const STATES: CustomSymbolModel = {
  ...AGITATOR,
  name: "states",
  properties: { ...AGITATOR.properties, state: "number" },
  parts: [
    { part: "rotor", stroke: "running" },
    { part: "liquid", travel: "level", range: [0, 100], move: [0, -20] },
    { part: "reading", equals: "state", value: 2 },
  ],
  svg: JSON.parse(JSON.stringify(AGITATOR.svg).replace(/my-agitator-/g, "my-states-")),
};

/** A heater element drawn as a line in a group without fill, and a filled lamp, both lit while the
 * heater runs (§14.4 item 2). */
const HEATER_ELEMENT: CustomSymbolModel = {
  name: "element",
  title: "Element",
  size: [128, 108],
  viewBox: [0, 0, 64, 64],
  properties: { running: "boolean" },
  ports: [],
  parts: [
    { part: "coil", active: "running" },
    { part: "lamp", active: "running" },
  ],
  svg: {
    tag: "g",
    attrs: { fill: "none", stroke: "#888" },
    children: [
      {
        tag: "path",
        attrs: { id: "my-element-coil", "data-part": "coil", d: "M4 30 H40 A4 4 0 0 1 40 38 H4" },
        children: [],
      },
      {
        tag: "circle",
        attrs: { id: "my-element-lamp", "data-part": "lamp", cx: "56", cy: "8", r: "4", fill: "#333" },
        children: [],
      },
    ],
  },
};

suite("HMI drawing: My Symbols", () => {
  suiteSetup(() => registerSymbols([AGITATOR, STATES, HEATER_ELEMENT]));

  test("an active part drawn as a line is painted on its line, a filled one on its fill (§14.4 item 2)", () => {
    const element: HmiObjectModel = {
      id: "eh",
      type: "my:element",
      at: [0, 0],
      size: [128, 108],
      rotation: 0,
      bind: { running: { path: "X.running", direction: "read" } },
    };
    const running = drawObject(element, valuesOf({ running: true }));
    const coil = part(running, "coil");
    assert.ok(coil?.attrs.stroke && coil.attrs.stroke !== "#888", renderToString(running));
    assert.strictEqual(coil?.attrs.fill, undefined, "the outline is not filled");
    assert.strictEqual(coil?.attrs.filter, "url(#hmi-glow)", "it glows while lit");
    assert.notStrictEqual(part(running, "lamp")?.attrs.fill, "#333", "the lamp's fill is lit");
    const stopped = drawObject(element, valuesOf({ running: false }));
    assert.strictEqual(part(stopped, "coil")?.attrs.stroke, undefined);
    assert.strictEqual(part(stopped, "coil")?.attrs.filter, undefined, "no glow while off");
  });

  test("a project's symbol moves a part to and fro, along its travel, or shows it in one state only", () => {
    const object = { ...mixer(["running", "level", "state"]), type: "my:states" };
    const drawn = drawObject(object, valuesOf({ running: true, level: 50, state: 2 }));
    assert.strictEqual(part(drawn, "rotor")?.attrs.class, "hmi-stroke", renderToString(drawn));
    assert.match(part(drawn, "liquid")?.attrs.style ?? "", /translate\(0px, -10px\)/, "half its travel of 20 upwards");
    assert.strictEqual(part(drawn, "reading")?.attrs.display, undefined, "shown in state 2");
    const other = drawObject(object, valuesOf({ running: false, level: 0, state: 1 }));
    assert.strictEqual(part(other, "rotor")?.attrs.class, undefined);
    assert.strictEqual(part(other, "reading")?.attrs.display, "none", "hidden in state 1");
  });

  test("a project's symbol draws its drawing, its parts driven by the bindings", () => {
    const object = mixer(["running", "level", "speed", "fault"]);
    const running = drawObject(object, valuesOf({ running: true, level: 40, speed: 120, fault: false }));
    assert.ok(part(running, "symbol"), renderToString(running));
    assert.strictEqual(part(running, "rotor")?.attrs.class, "hmi-spin", renderToString(running));
    assert.match(part(running, "liquid")?.attrs.style ?? "", /scaleY\(0\.4\)/);
    assert.strictEqual(words(part(running, "reading")), "120");
    assert.strictEqual(words(part(running, "tag")), "M-1");
    const stopped = drawObject(object, valuesOf({ running: false, level: 40, speed: 0, fault: true }));
    assert.strictEqual(part(stopped, "rotor")?.attrs.class, undefined);
    assert.strictEqual(stateWord(stopped), "Fault");
  });

  test("pipes reach a project's symbol's ports, and the editor offers only symbols it has", () => {
    const inlet = portPoint(mixer([]), "inlet");
    assert.ok(inlet, "the inlet is drawn");
    assert.strictEqual(inlet.direction, "left");
    assert.strictEqual(drawable("my:agitator"), true);
    assert.strictEqual(drawable("my:missing"), false);
  });
});
