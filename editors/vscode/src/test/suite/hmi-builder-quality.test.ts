import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import { colourWarnings, LanguageTable, objectWarnings } from "../../hmiBuilder/editor/quality";

// Specification 39, §14.1 item 4: the editor's quality checks, as warnings.

const CATALOGUE: Catalogue = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../../../../../crates/trust-runtime/src/hmi/catalogue/components.json"),
    "utf8",
  ),
);

function value(name: string, type: string, kinds: string[]): VariableNode {
  return { name, path: `Station.${name}`, kind: "value", type, kinds, writable: true };
}

const VARIABLES: VariableNode[] = [
  {
    name: "Station",
    path: "Station",
    kind: "program",
    type: "PumpStation",
    kinds: [],
    writable: false,
    children: [
      value("Speed", "REAL", ["number", "value"]),
      value("Flow", "LREAL", ["number", "value"]),
      value("Count", "DINT", ["number", "value"]),
      value("Start", "BOOL", ["boolean", "value"]),
      {
        name: "Tanks",
        path: "Station.Tanks",
        kind: "array",
        type: "ARRAY[1..3] OF Tank",
        kinds: [],
        writable: false,
        array: {
          dimensions: [[1, 3]],
          element: {
            name: "Tanks[1]",
            path: "Station.Tanks[1]",
            kind: "structure",
            type: "Tank",
            kinds: [],
            writable: false,
            children: [value("Level", "REAL", ["number", "value"])],
          },
        },
      },
    ],
  },
];

function object(id: string, type: string, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  const component = CATALOGUE.components.find((candidate) => candidate.type === type);
  return { id, type, at: [0, 0], size: component?.size ?? [160, 48], rotation: 0, bind: {}, ...extra };
}

function bound(
  id: string,
  type: string,
  property: string,
  variable: string,
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return object(id, type, { bind: { [property]: { path: variable, direction: "read" } }, ...extra });
}

function page(...objects: HmiObjectModel[]): HmiPageModel {
  return { title: "Checks", object: objects, pipe: [] };
}

const kinds = (warnings: Array<{ object?: string; kind: string }>): string[] =>
  warnings.map((warning) => `${warning.object}:${warning.kind}`);

suite("HMI Builder quality checks", () => {
  test("a control, a page button or an object with a click below 44 × 44 px is flagged", () => {
    const warnings = objectWarnings(
      page(
        bound("start", "button", "target", "Station.Start", { size: [120, 36], text: "Start" }),
        object("trends", "page-button", { size: [160, 40], text: "Trends", click: { page: "trends" } }),
        bound("speed", "value", "value", "Station.Speed", { size: [160, 40], unit: "rpm" }),
        bound("detail", "value", "value", "Station.Speed", { size: [40, 40], unit: "rpm", click: { popup: "pump" } }),
        bound("stop", "button", "target", "Station.Start", { size: [120, 48], text: "Stop" }),
      ),
      CATALOGUE,
      VARIABLES,
    );
    const touch = warnings.filter((warning) => warning.kind === "touch-target");
    assert.deepStrictEqual(
      touch.map((warning) => warning.object),
      ["start", "trends", "detail"],
    );
    assert.match(touch[0].message, /120 × 36 px/);
    assert.match(touch[0].message, /44 × 44 px/);
  });

  test('a number display or entry bound to a REAL or LREAL without a unit is flagged; unit = "" says it has none', () => {
    const warnings = objectWarnings(
      page(
        bound("speed", "value", "value", "Station.Speed"),
        bound("flow", "gauge", "value", "Station.Flow"),
        bound("level", "bar", "value", "Station.Tanks[2].Level"),
        bound("count", "value", "value", "Station.Count"),
        bound("withUnit", "value", "value", "Station.Speed", { unit: "rpm" }),
        bound("none", "value", "value", "Station.Speed", { unit: "" }),
        bound("unknownPath", "value", "value", "Station.Gone"),
      ),
      CATALOGUE,
      VARIABLES,
    );
    assert.deepStrictEqual(
      warnings.filter((warning) => warning.kind === "unit").map((warning) => warning.object),
      ["speed", "flow", "level"],
    );
  });

  test("an object whose component takes bindings but has none is flagged; static text, layout and drawings are not", () => {
    const warnings = objectWarnings(
      page(
        object("lonely", "value", { label: "Level" }),
        object("trend", "trend"),
        object("title", "text", { text: "Feed station" }),
        object("frame", "panel"),
        object("log", "event-log"),
        object("vessel", "tank"),
      ),
      CATALOGUE,
      VARIABLES,
    );
    assert.deepStrictEqual(
      warnings.filter((warning) => warning.kind === "unbound").map((warning) => warning.object),
      ["lonely", "trend"],
    );
  });

  test("an object whose label is cut to fit is flagged", () => {
    const warnings = objectWarnings(
      page(
        bound("long", "value", "value", "Station.Speed", {
          size: [160, 48],
          label: "Feed pump P101 speed",
          unit: "rpm",
        }),
        bound("short", "value", "value", "Station.Speed", { size: [160, 48], label: "Speed", unit: "rpm" }),
      ),
      CATALOGUE,
      VARIABLES,
    );
    const cut = warnings.filter((warning) => warning.kind === "cut-text");
    assert.deepStrictEqual(
      cut.map((warning) => warning.object),
      ["long"],
    );
    assert.match(cut[0].message, /wider|shorter/);
  });

  test("a text cut in any of the project's languages is flagged with the language (§14.5 item 6)", () => {
    const start = {
      id: "start",
      type: "text",
      at: [0, 0] as [number, number],
      size: [120, 40] as [number, number],
      rotation: 0,
      bind: {},
      text: "Start",
    };
    const languages: LanguageTable[] = [
      { tag: "en", name: "English", texts: {} },
      { tag: "de", name: "Deutsch", texts: { Start: "Pumpe jetzt sofort starten" } },
      { tag: "sv", name: "Svenska", texts: { Start: "Starta" } },
    ];
    const cut = objectWarnings(page(start), CATALOGUE, VARIABLES, languages).filter(
      (warning) => warning.kind === "cut-text",
    );
    assert.strictEqual(cut.length, 1, JSON.stringify(cut));
    assert.strictEqual(cut[0].object, "start");
    assert.match(cut[0].message, /Deutsch/);
    assert.match(cut[0].message, /Pumpe jetzt sofort starten/);
  });

  test("one of truST's own words cut in a language is flagged too (§14.5 item 6)", () => {
    const pump = bound("pump", "pump", "running", "Station.Start", { tag: "P-101" });
    const languages: LanguageTable[] = [
      { tag: "en", name: "English", texts: {} },
      { tag: "sv", name: "Svenska", texts: { Running: "Pumpen går för fullt hela tiden" } },
    ];
    const cut = objectWarnings(page(pump), CATALOGUE, VARIABLES, languages).filter(
      (warning) => warning.kind === "cut-text",
    );
    assert.deepStrictEqual(
      cut.map((warning) => warning.object),
      ["pump"],
    );
    assert.match(cut[0].message, /Svenska/);
    assert.match(cut[0].message, /Pumpen går för fullt hela tiden/);
  });

  test("the shipped colours raise no colour warnings", () => {
    assert.deepStrictEqual(colourWarnings({}, CATALOGUE), []);
  });

  test("a project colour that takes text below its contrast is flagged at the colour, with the ratio", () => {
    const warnings = colourWarnings({ dark: { muted: "#1f2430" } }, CATALOGUE);
    const contrast = warnings.filter((warning) => warning.kind === "contrast");
    assert.deepStrictEqual(
      contrast.map((warning) => `${warning.colour?.theme}.${warning.colour?.token}`),
      ["dark.muted"],
    );
    assert.match(contrast[0].message, /\d+(\.\d+)?:1/);
    assert.match(contrast[0].message, /4\.5:1|3:1/);
  });

  test("a project colour that looks like the warning or an alarm colour is flagged", () => {
    const warnings = colourWarnings(
      { dark: { accent: "#f97316", "pen-3": "#ff5470" }, light: { "pen-1": "#0e7490" } },
      CATALOGUE,
    );
    const alike = warnings.filter((warning) => warning.kind === "look-alike");
    assert.deepStrictEqual(
      alike.map((warning) => `${warning.colour?.theme}.${warning.colour?.token}`),
      ["dark.accent", "dark.pen-3"],
    );
    assert.match(alike[0].message, /alarm/);
  });
});
