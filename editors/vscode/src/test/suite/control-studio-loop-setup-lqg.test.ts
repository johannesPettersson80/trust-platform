import * as assert from "assert";
import { formModelOf } from "../../controlStudio/loopSetup/loopSetupPanel";
import { open } from "./control-studio-loop-setup-form-harness";


/** Lesson 67's one-second model of the cascade around 0.25 m; only tank 2 is measured. */



const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** The estimator as the form posts it (B1.3, C4): every estimate row the template draws (the engine's limit, three), each number as the
 * text typed; the engine reads one row per state of the model and leaves the third. */
const estimator = { modelFile: "lqg-model.json", measurementNoise: "1e-6",
  states: [{ estimate: "P1.X1Hat", gain: "P1.K1", observerGain: "P1.L1", processNoise: "1e-6" }, { estimate: "P1.X2Hat", gain: "P1.K2", observerGain: "P1.L2", processNoise: "1e-6" },
    { estimate: "", gain: "", observerGain: "", processNoise: "" }] };
/** A further-output row of the measured design, which the form posts blank for LQG too; the engine reads no further output for LQG. */
const blankFurtherOutput = { signal: { symbol: "", unit: "", minimum: "", maximum: "" }, gain: "" };
const draft = { ...scope, name: "Tank 2 level", action: "direct", family: "LQG", instance: "P1",
  manipulatedVariable: { symbol: "P1.PumpM3PerS", unit: "m3/s", minimum: "0", maximum: "0.02" },
  processVariable: { symbol: "P1.Tank2M", unit: "m", minimum: "0", maximum: "0.5" }, setpoint: { symbol: "P1.Tank2SetpointM" },
  gain: "", referenceGain: "P1.N", states: [blankFurtherOutput, blankFurtherOutput, blankFurtherOutput],
  mode: { symbol: "P1.Enabled", whenTrue: "Auto" }, manualOutput: "P1.ManualM3PerS", estimator };

suite("Control Studio LQG setup", () => {

  test("sets up LQG in the form from the program's estimates, gains and the estimator's model file", () => {
    const signals = ["P1.ManualM3PerS", "P1.PumpM3PerS", "P1.Tank2M", "P1.Tank2SetpointM"];
    const view = open({ structure: "identify", pids: [], prefill: undefined, prefills: [], loopNames: [], records: [], instances: ["P1"], writes: [], initializes: [],
      addressedReal: signals, measurements: signals, signals, booleans: ["P1.Enabled"], modes: ["P1.Enabled"],
      reals: ["P1.K1", "P1.K2", "P1.L1", "P1.L2", "P1.ManualM3PerS", "P1.N", "P1.PumpM3PerS", "P1.Tank2M", "P1.Tank2SetpointM", "P1.X1Hat", "P1.X2Hat"],
      estimatorModels: [{ file: "lqg-model.json", states: 2 }] });
    try {
      view.set("structure", "state-feedback");
      const families = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="feedback.family"] option')).map(option => option.value);
      assert.ok(families.includes("LQG"), `LQG is a state-feedback design: ${families}`);
      for (const [field, value] of [["name", "Tank 2 level"], ["feedback.family", "LQG"], ["feedback.instance", "P1"], ["feedback.manipulatedVariable", "P1.PumpM3PerS"],
        ["feedback.manipulatedVariable.unit", "m3/s"], ["feedback.manipulatedVariable.minimum", "0"], ["feedback.manipulatedVariable.maximum", "0.02"],
        ["feedback.processVariable", "P1.Tank2M"], ["feedback.processVariable.unit", "m"], ["feedback.processVariable.minimum", "0"],
        ["feedback.processVariable.maximum", "0.5"], ["feedback.setpoint", "P1.Tank2SetpointM"], ["feedback.referenceGain", "P1.N"],
        ["feedback.mode", "P1.Enabled"], ["feedback.manualOutput", "P1.ManualM3PerS"]]) view.set(field, value);
      assert.deepStrictEqual(Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="feedback.estimatorModel"] option')).map(option => option.value),
        ["", "lqg-model.json"], "the form offers the model files the engine answers");
      view.set("feedback.estimatorModel", "lqg-model.json");
      assert.deepStrictEqual(Array.from(view.root.querySelectorAll<HTMLElement>("[data-estimate-state]")).map(row => row.hidden), [false, false, true],
        "one row per state of the chosen model");
      for (const [field, value] of [["feedback.estimate.0", "P1.X1Hat"], ["feedback.estimate.0.gain", "P1.K1"], ["feedback.estimate.0.observerGain", "P1.L1"],
        ["feedback.estimate.0.processNoise", "1e-6"], ["feedback.estimate.1", "P1.X2Hat"], ["feedback.estimate.1.gain", "P1.K2"],
        ["feedback.estimate.1.observerGain", "P1.L2"], ["feedback.estimate.1.processNoise", "1e-6"], ["feedback.measurementNoise", "1e-6"]]) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "");
      save.click();
      // The whole draft as the engine reads it: the fixture's fields, unscoped, under the state-feedback structure (C4).
      const { projectId: _project, runtimeId: _runtime, ...unscoped } = draft;
      assert.deepStrictEqual(view.posts[0]?.draft, { structure: "state-feedback", ...unscoped });
    } finally { view.close(); }
  });

  test("offers the model files the engine answers, beside the form's model", () => {
    const model = formModelOf({ formModel: { structure: "identify" }, loops: [], rejected: [], estimatorModels: [{ file: "lqg-model.json", states: 2 }] } as any);
    assert.deepStrictEqual(model.estimatorModels, [{ file: "lqg-model.json", states: 2 }]);
  });
});
