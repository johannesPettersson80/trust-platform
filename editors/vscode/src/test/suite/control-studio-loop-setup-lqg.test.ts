import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { open } from "./control-studio-loop-setup-form-harness";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 1000 }];
const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** Two tanks in series; the program measures only tank 2 and estimates both levels (Specification 35 §17.1, §17.8 G). */
const tanks = {
  symbols: [
    { symbol: "P1.Tank2M", type: "LREAL", address: "%IL8" }, { symbol: "P1.PumpM3PerS", type: "LREAL", address: "%QL0" },
    { symbol: "P1.Tank2SetpointM", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualM3PerS", type: "LREAL", address: "%ML8" },
    { symbol: "P1.Enabled", type: "BOOL" }, { symbol: "P1.K1", type: "LREAL" }, { symbol: "P1.K2", type: "LREAL" }, { symbol: "P1.N", type: "LREAL" },
    { symbol: "P1.L1", type: "LREAL" }, { symbol: "P1.L2", type: "LREAL" }, { symbol: "P1.X1Hat", type: "LREAL" }, { symbol: "P1.X2Hat", type: "LREAL" },
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }], links: [], writes: ["P1.PumpM3PerS", "P1.X1Hat", "P1.X2Hat"],
};
/** Lesson 67's one-second model of the cascade around 0.25 m; only tank 2 is measured. */
const estimatorModel: any = { kind: "discrete-state-space", revision: "two-tanks", source: "provided", validation: "validated", evidence: ["lesson"],
  delaySeconds: 0, sampleTimeSeconds: 1, inputs: [{ id: "pump-flow", unit: "m3/s", operatingRange: { minimum: 0, maximum: 0.02 } }],
  outputs: [{ id: "tank-2-level", unit: "m", operatingRange: { minimum: 0.05, maximum: 0.45 } }],
  states: [{ id: "tank-1-deviation", unit: "m", operatingRange: { minimum: -0.2, maximum: 0.2 } }, { id: "tank-2-deviation", unit: "m", operatingRange: { minimum: -0.2, maximum: 0.2 } }],
  a: [[0.96, 0], [0.04, 0.96]], b: [[2], [0]], c: [[0, 1]], d: [[0]], operatingPoint: { inputs: [0.01], outputs: [0.25] } };
const estimator = { modelFile: "lqg-model.json", measurementNoise: 1e-6,
  states: [{ estimate: "P1.X1Hat", gain: "P1.K1", observerGain: "P1.L1", processNoise: 1e-6 }, { estimate: "P1.X2Hat", gain: "P1.K2", observerGain: "P1.L2", processNoise: 1e-6 }] };
const draft = { ...scope, name: "Tank 2 level", action: "direct", family: "LQG", instance: "P1",
  manipulatedVariable: { symbol: "P1.PumpM3PerS", unit: "m3/s", minimum: 0, maximum: 0.02 },
  processVariable: { symbol: "P1.Tank2M", unit: "m", minimum: 0, maximum: 0.5 }, setpoint: { symbol: "P1.Tank2SetpointM" },
  referenceGain: "P1.N", mode: { symbol: "P1.Enabled", whenTrue: "Auto" }, manualOutput: "P1.ManualM3PerS",
  experimentStep: 0.002, experimentDurationSeconds: 300, estimator };

suite("Control Studio LQG setup", () => {
  test("writes an LQG loop whose program estimates the states with the chosen model, ready for LQG design", () => {
    const api = setup();
    const loop = api.stateFeedbackLoopDefinition(tanks, { ...draft, estimator: { ...estimator, model: estimatorModel } });
    const validation: any = validateControlLoopDefinition(loop, scope);
    assert.strictEqual(validation.kind, "accepted", JSON.stringify(validation.reason ?? validation));
    const feedback = loop.controller.stateFeedback;
    assert.deepStrictEqual([feedback.family, feedback.gainSymbols, feedback.referenceGainSymbols, feedback.observerGainSymbols, feedback.estimatorStateSymbols],
      ["LQG", [["P1.K1", "P1.K2"]], [["P1.N"]], [["P1.L1"], ["P1.L2"]], ["P1.X1Hat", "P1.X2Hat"]]);
    assert.deepStrictEqual(feedback.estimatorModel, { a: estimatorModel.a, b: estimatorModel.b, c: estimatorModel.c, sampleTimeSeconds: 1 });
    assert.deepStrictEqual([feedback.processNoise, feedback.measurementNoise], [[[1e-6, 0], [0, 1e-6]], [[1e-6]]]);
    assert.deepStrictEqual(feedback.initialization.states.map((state: any) => [state.symbol, state.initial, state.unit]),
      [["P1.X1Hat", 0, "m"], ["P1.X2Hat", 0, "m"]], "the estimates start at the operating point");
    assert.deepStrictEqual(loop.channels.outputs.map((output: any) => output.modelChannelId), ["tank-2-level"], "only the controlled output is measured");
    assert.deepStrictEqual(loop.channels.inputs.map((input: any) => input.modelChannelId), ["pump-flow"], "the supplied model maps by its own channels");
    const { designStateController } = require("../../controlStudio/stateControllerDesign");
    const design = designStateController(loop, estimatorModel, 60);
    assert.deepStrictEqual(Object.keys(design.candidate.parameters).sort(), ["P1.K1", "P1.K2", "P1.L1", "P1.L2", "P1.N"]);
    assert.ok(design.closedLoopPoles.every((pole: any) => Math.hypot(pole.real, pole.imaginary) < 1), "the designed gains stabilize the model");
  });

  test("sets up LQG in the form from the program's estimates, gains and the estimator's model file", () => {
    const view = open({ structure: "identify", pids: [], prefill: undefined, prefills: [], records: [], instances: ["P1"], writes: [], initializes: [],
      addressedReal: ["P1.ManualM3PerS", "P1.PumpM3PerS", "P1.Tank2M", "P1.Tank2SetpointM"], booleans: ["P1.Enabled"], modes: ["P1.Enabled"],
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
        ["feedback.mode", "P1.Enabled"], ["feedback.manualOutput", "P1.ManualM3PerS"], ["feedback.experimentDuration", "300"]]) view.set(field, value);
      const next = view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "";
      assert.match(next, /estimator.*model|model.*estimator/i, `the estimator's model file is next: ${next}`);
      view.set("feedback.estimatorModel", "lqg-model.json");
      assert.deepStrictEqual(Array.from(view.root.querySelectorAll<HTMLElement>("[data-estimate-state]")).map(row => row.hidden), [false, false, true],
        "one row per state of the chosen model");
      for (const [field, value] of [["feedback.estimate.0", "P1.X1Hat"], ["feedback.estimate.0.gain", "P1.K1"], ["feedback.estimate.0.observerGain", "P1.L1"],
        ["feedback.estimate.0.processNoise", "1e-6"], ["feedback.estimate.1", "P1.X2Hat"], ["feedback.estimate.1.gain", "P1.K2"],
        ["feedback.estimate.1.observerGain", "P1.L2"], ["feedback.estimate.1.processNoise", "1e-6"], ["feedback.measurementNoise", "1e-6"]]) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts[0]?.draft?.estimator, estimator);
      assert.strictEqual(view.posts[0]?.draft?.family, "LQG");
    } finally { view.close(); }
  });

  test("offers the project's model files an estimator can implement", async () => {
    const fs = require("fs"), os = require("os"), path = require("path");
    let api: any;
    try { api = require("../../controlStudio/loopSetup/estimatorModelFiles"); } catch { api = {}; }
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-lqg-files-"));
    try {
      fs.writeFileSync(path.join(root, "lqg-model.json"), JSON.stringify(estimatorModel));
      fs.writeFileSync(path.join(root, "notes.json"), JSON.stringify({ title: "not a model" }));
      fs.writeFileSync(path.join(root, "two-inputs.json"), JSON.stringify({ ...estimatorModel, inputs: [...estimatorModel.inputs, { id: "second", unit: "m3/s", operatingRange: { minimum: 0, maximum: 1 } }],
        b: [[2, 0], [0, 0]], d: [[0, 0]], operatingPoint: { inputs: [0.01, 0], outputs: [0.25] } }));
      fs.mkdirSync(path.join(root, "models"));
      fs.writeFileSync(path.join(root, "models", "tanks.json"), JSON.stringify(estimatorModel));
      assert.strictEqual(typeof api.estimatorModelFiles, "function", "the setup form lists the project's model files");
      assert.deepStrictEqual(await api.estimatorModelFiles(root), [{ file: "lqg-model.json", states: 2 }, { file: "models/tanks.json", states: 2 }]);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });

  test("saves the LQG loop with the model file's estimator model", async () => {
    const fs = require("fs"), os = require("os"), path = require("path");
    const { saveLoopSetup } = require("../../controlStudio/loopSetup/loopSetupSave");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-lqg-setup-"));
    try {
      fs.writeFileSync(path.join(root, "lqg-model.json"), JSON.stringify(estimatorModel));
      const { projectId: _project, runtimeId: _runtime, ...unscoped } = draft;
      const requests = await saveLoopSetup(root, scope, tanks, { structure: "state-feedback", ...unscoped });
      assert.deepStrictEqual(requests[0].loopDefinition.controller.stateFeedback.estimatorModel?.a, estimatorModel.a);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
