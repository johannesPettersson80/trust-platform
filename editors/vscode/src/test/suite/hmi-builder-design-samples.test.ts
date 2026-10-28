import * as assert from "assert";

import type { HmiObjectModel, LiveValue, StructuredValue } from "../../hmiBuilder/drawing/model";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import { designValues } from "../../hmiBuilder/editor/designSamples";

// Specification 39, §3.4: design samples while no source is connected.

const CATALOGUE: Catalogue = {
  format: 1,
  components: [
    {
      type: "value",
      category: "display",
      phase: 1,
      size: [160, 48],
      properties: { value: { accepts: ["value"], directions: ["read"] } },
    },
    {
      type: "status",
      category: "display",
      phase: 1,
      size: [160, 40],
      properties: { state: { accepts: ["boolean", "number"], directions: ["read"] } },
    },
    {
      type: "text",
      category: "display",
      phase: 1,
      size: [160, 32],
      properties: { text: { accepts: ["value"], directions: ["read"] } },
    },
    {
      type: "table",
      category: "display",
      phase: 1,
      size: [320, 160],
      properties: { value: { accepts: ["array", "structure"], directions: ["read"] } },
    },
    {
      type: "trend",
      category: "display",
      phase: 1,
      size: [480, 240],
      properties: { "high-limit": { accepts: ["number"], directions: ["read"] } },
      additional: { accepts: ["number", "boolean"], directions: ["read"] },
    },
    {
      type: "switch",
      category: "control",
      phase: 1,
      size: [120, 40],
      properties: { target: { accepts: ["boolean"], directions: ["write", "read-write"] } },
    },
    {
      type: "selector",
      category: "control",
      phase: 1,
      size: [160, 40],
      properties: { value: { accepts: ["enumeration", "number"], directions: ["read-write", "write"] } },
    },
    {
      type: "mode-selector",
      category: "control",
      phase: 1,
      size: [240, 40],
      properties: { mode: { accepts: ["enumeration"], directions: ["read-write", "write"] } },
    },
    {
      type: "control-valve",
      category: "equipment",
      phase: 2,
      size: [128, 108] as [number, number],
      properties: {
        position: { accepts: ["number"], directions: ["read"] },
        fault: { accepts: ["boolean"], directions: ["read"] },
      },
    },
    {
      type: "tank",
      category: "equipment",
      phase: 2,
      size: [128, 172] as [number, number],
      properties: { level: { accepts: ["number"], directions: ["read"] } },
    },
    ...(["pump", "valve"] as const).map((type) => ({
      type,
      category: "equipment",
      phase: 2,
      size: [100, 100] as [number, number],
      properties: Object.fromEntries(
        ["running", "open", "closed", "fault", "manual", "out-of-service"].map((name) => [
          name,
          { accepts: ["boolean"], directions: ["read"] },
        ]),
      ),
    })),
  ],
};

function bound(type: string, bind: Record<string, string>, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  const bindings = Object.fromEntries(
    Object.entries(bind).map(([name, path]) => [name, { path, direction: "read" as const }]),
  );
  return { id: `${type}-1`, type, at: [0, 0], size: [100, 40], rotation: 0, bind: bindings, ...extra };
}

function sampleOf(values: Record<string, LiveValue>, path: string): StructuredValue {
  const sample = values[path];
  assert.ok(sample, `a sample for ${path} in ${JSON.stringify(values)}`);
  return sample.value;
}

suite("HMI Builder editor: design samples", () => {
  test("a number is drawn at 60 % of its range, or of 0 to 100", () => {
    const values = designValues(
      {
        title: "Design",
        object: [bound("value", { value: "P1.Speed" }, { min: 0, max: 1500 }), bound("value", { value: "P1.Level" })],
        pipe: [],
      },
      CATALOGUE,
    );
    assert.deepStrictEqual(values["P1.Speed"], { value: 900, state: "confirmed" });
    assert.deepStrictEqual(values["P1.Level"], { value: 60, state: "confirmed" });
  });

  test("equipment shows its normal state: a pump running, a valve open, and no fault, manual or out of service (§5.3)", () => {
    const values = designValues(
      {
        title: "Design",
        object: [
          bound("pump", {
            running: "P1.Running",
            fault: "P1.Fault",
            manual: "P1.Manual",
            "out-of-service": "P1.OutOfService",
          }),
          bound("valve", { open: "V1.Open", closed: "V1.Closed", fault: "V1.Fault" }),
        ],
        pipe: [],
      },
      CATALOGUE,
    );
    assert.strictEqual(sampleOf(values, "P1.Running"), true);
    assert.strictEqual(sampleOf(values, "V1.Open"), true);
    for (const off of ["P1.Fault", "P1.Manual", "P1.OutOfService", "V1.Closed", "V1.Fault"])
      assert.strictEqual(sampleOf(values, off), false, off);
  });

  test("a symbol shows its own sample state: a tank half full of its range (§5.3 item 6)", () => {
    const values = designValues(
      { title: "Design", object: [bound("tank", { level: "T1.Level" }, { min: 0, max: 4 })], pipe: [] },
      CATALOGUE,
    );
    assert.strictEqual(sampleOf(values, "T1.Level"), 2);
  });

  test("a control valve's position samples at 60 % of its travel, as other numbers do", () => {
    const values = designValues(
      { title: "Design", object: [bound("control-valve", { position: "FV.Position", fault: "FV.Fault" })], pipe: [] },
      CATALOGUE,
    );
    assert.strictEqual(sampleOf(values, "FV.Position"), 60);
    assert.strictEqual(sampleOf(values, "FV.Fault"), false);
  });

  test("a state is on, a text is its variable's name, and a table is empty", () => {
    const values = designValues(
      {
        title: "Design",
        object: [
          bound("status", { state: "P1.Running" }),
          bound("switch", { target: "P1.Enable" }),
          bound("text", { text: "P1.Recipe.Name" }),
          bound("table", { value: "P1.Params" }),
        ],
        pipe: [],
      },
      CATALOGUE,
    );
    assert.strictEqual(sampleOf(values, "P1.Running"), true);
    assert.strictEqual(sampleOf(values, "P1.Enable"), true);
    assert.strictEqual(sampleOf(values, "P1.Recipe.Name"), "Name");
    assert.deepStrictEqual(sampleOf(values, "P1.Params"), []);
  });

  test("a faceplate's field samples as the kind it takes, and a text field shows its name without the $ (§8.7 item 6)", () => {
    const values = designValues(
      {
        title: "Pump",
        size: [400, 300],
        fields: {
          Running: { accepts: ["boolean"] },
          Tag: { accepts: ["text"] },
          Speed: { accepts: ["number"] },
          Any: { accepts: [] },
        },
        object: [
          bound("value", { value: "$Running" }),
          bound("text", { text: "$Tag" }),
          bound("value", { value: "$Speed" }, { id: "speed" }),
          bound("value", { value: "$Any" }, { id: "any" }),
        ],
        pipe: [],
      },
      CATALOGUE,
    );
    assert.strictEqual(sampleOf(values, "$Running"), true, "a boolean field is on");
    assert.strictEqual(sampleOf(values, "$Tag"), "Tag");
    assert.strictEqual(sampleOf(values, "$Speed"), 60);
    assert.strictEqual(sampleOf(values, "$Any"), 60, "a field taking any value samples as its property does");
  });

  test("an explicit trend range supplies the shared sample before an unscaled symbol", () => {
    const trend = bound("trend", { Level: "Tank.Level", Pressure: "Tank.Pressure" }, { min: 0, max: 3 });
    trend.bind.Pressure.range = [10, 20];
    const values = designValues(
      {
        title: "Preview",
        pipe: [],
        object: [bound("value", { value: "Tank.Level" }), bound("value", { value: "Tank.Pressure" }), trend],
      },
      CATALOGUE,
    );
    assert.strictEqual(
      sampleOf(values, "Tank.Level"),
      1.8,
      "the shared sample stays inside the authored physical range",
    );
    assert.strictEqual(sampleOf(values, "Tank.Pressure"), 16, "each pen uses its own range");
  });

  test("a trend's pens and limits get number samples and no history", () => {
    const values = designValues(
      {
        title: "Design",
        object: [bound("trend", { pv: "P1.PV", "high-limit": "P1.High" }, { min: 0, max: 200 })],
        pipe: [],
      },
      CATALOGUE,
    );
    assert.deepStrictEqual([sampleOf(values, "P1.PV"), sampleOf(values, "P1.High")], [120, 120]);
  });

  test("a selector shows its first option and a mode selector Auto", () => {
    const values = designValues(
      {
        title: "Design",
        object: [
          bound("selector", { value: "P1.Speed" }, { options: ["Low", "High"] }),
          bound("mode-selector", { mode: "P1.Mode" }),
          bound("mode-selector", { mode: "P2.Mode" }, { options: ["Manual"] }),
        ],
        pipe: [],
      },
      CATALOGUE,
    );
    assert.deepStrictEqual(values["P1.Speed"], { value: "Low", state: "confirmed" });
    assert.deepStrictEqual(values["P1.Mode"], { value: "Auto", state: "confirmed" });
    assert.deepStrictEqual(values["P2.Mode"], { value: "Manual", state: "confirmed" });
  });
});
