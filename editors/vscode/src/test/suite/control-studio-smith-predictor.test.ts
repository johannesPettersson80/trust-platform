import * as assert from "assert";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

/** A heater whose program feeds its library PID the measurement plus a Smith predictor's correction (Specification 35 §17.13). */
const heater = {
  symbols: [
    { symbol: "P1.WaterC", type: "LREAL", address: "%IL0" },
    { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" },
    { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.ManualPct", type: "LREAL", address: "%ML16" },
    { symbol: "P1.Auto", type: "BOOL" },
    { symbol: "P1.ModelGain", type: "LREAL" },
    { symbol: "P1.ModelTimeConstantS", type: "LREAL" },
    { symbol: "P1.ModelDelayS", type: "LREAL" },
    { symbol: "P1.CorrectionC", type: "LREAL" },
    { symbol: "P1.PredictedC", type: "LREAL" },
    ...pidSymbols("P1.Loop", "P1.Parameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Loop", type: "Control.PID", tasks: TASK }],
  links: [
    { instance: "P1.Loop", method: "Configure", argument: "Parameters", record: "P1.Parameters", output: null, fields: {} },
    { instance: "P1.Loop", method: "Update", argument: "Input", record: "P1.Input", output: "P1.HeaterPct",
      fields: { ManualOutput: ["P1.ManualPct"], ProcessValue: ["P1.PredictedC"], Setpoint: ["P1.SetpointC"] } },
  ],
  writes: ["P1.PredictedC", "P1.CorrectionC", "P1.HeaterPct"],
};
const draft = () => ({ ...scope, name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
  processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: 0, maximum: 80 },
  manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  model: { gain: "P1.ModelGain", timeConstant: "P1.ModelTimeConstantS", delay: "P1.ModelDelayS", correction: "P1.CorrectionC" },
  experimentStep: 10, experimentDurationSeconds: 60 });
/** 0.2 °C per % through a 5 s lag after a 1 s delay, at rest at 35 °C with 75 %. */
const model: any = { kind: "transfer-function", numerator: [0.2], denominator: [5, 1], delaySeconds: 1, revision: "heater", source: "fitted",
  validation: "validated", evidence: ["recording"], inputs: [{ id: "P1.HeaterPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "P1.WaterC", unit: "degC", operatingRange: { minimum: 0, maximum: 80 } }], operatingPoint: { inputs: [75], outputs: [35] } };
const task = { referenceChange: 0.5, maximumError: 0.1, settlingBand: 0.05, settlingTimeSeconds: 10, durationSeconds: 30 };
function smithLoop(): any {
  const api = setup();
  assert.strictEqual(typeof api.smithPredictorLoopDefinition, "function", "a Smith predictor needs its own generator");
  return api.smithPredictorLoopDefinition(heater, draft());
}
function near(actual: number, expected: number, tolerance: number, label: string): void {
  assert.ok(Math.abs(actual - expected) <= tolerance, `${label}: ${actual}, expected ${expected} ± ${tolerance}`);
}

suite("Control Studio Smith predictor", () => {
  test("writes one loop whose program's model values Tune sets and whose correction starts at zero", () => {
    const loop = smithLoop();
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
    const validation: any = validateControlLoopDefinition(loop, scope);
    assert.strictEqual(validation.kind, "accepted", JSON.stringify(validation.reason ?? validation));
    const project = loop.controller.project;
    assert.deepStrictEqual([project.family, project.seed], ["Smith", { kind: "smith-pid", family: "PI",
      proportionalSymbol: "P1.Parameters.ProportionalGain", integralSymbol: "P1.Parameters.IntegralGain", derivativeSymbol: "P1.Parameters.DerivativeGain",
      gainSymbol: "P1.ModelGain", timeConstantSymbol: "P1.ModelTimeConstantS", delaySymbol: "P1.ModelDelayS" }]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.slice(3).map((parameter: any) => [parameter.symbol, parameter.unit]),
      [["P1.ModelGain", "degC/%"], ["P1.ModelTimeConstantS", "s"], ["P1.ModelDelayS", "s"]]);
    const correction = project.initialization.states.find((state: any) => state.symbol === "P1.CorrectionC");
    assert.deepStrictEqual([correction?.initial, correction?.unit], [0, "degC"]);
    assert.strictEqual(loop.signals.processVariable.symbol, "P1.WaterC", "requirements and identification use the process measurement");
    assert.strictEqual(project.objective.requirementId, "target-error");
  });

  test("sets the model values from the model and designs the PI so the loop meets the requirements through the predictor", () => {
    const loop = smithLoop();
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const values = projectControllerSeed(loop, model, task, loop.controller.project.seed);
    near(values["P1.ModelGain"], 0.2, 0.002, "K"); near(values["P1.ModelTimeConstantS"], 5, 0.05, "T"); near(values["P1.ModelDelayS"], 1, 0.02, "θ");
    const kp = values["P1.Parameters.ProportionalGain"], ki = values["P1.Parameters.IntegralGain"];
    assert.ok(kp > 0 && ki > 0 && values["P1.Parameters.DerivativeGain"] === 0, JSON.stringify(values));
    // The plant with its delay, the predictor with the designed model and the PI: the step must settle by 10 s within 0.1 °C.
    const h = 0.01, delay = Math.round(1 / h), modelDelay = Math.round(values["P1.ModelDelayS"] / h);
    const plantHistory = Array(delay).fill(0), outputHistory = Array(modelDelay).fill(0);
    let y = 0, correction = 0, integral = 0, error = 0;
    for (let step = 0; step * h < task.durationSeconds; step++) {
      const e = task.referenceChange - (y + correction);
      const u = kp * e + integral;
      integral += ki * e * h;
      plantHistory.push(u); const applied = plantHistory.shift();
      outputHistory.push(u); const earlier = outputHistory.shift();
      y += h * (0.2 * applied - y) / 5;
      correction += h * (values["P1.ModelGain"] * (u - earlier) - correction) / values["P1.ModelTimeConstantS"];
      if (step * h >= task.settlingTimeSeconds) error = Math.max(error, Math.abs(y - task.referenceChange));
    }
    assert.ok(error <= task.maximumError, `${error} °C from the target after 10 s; 0.1 °C is allowed`);
  });

  test("refuses a settling time no longer than the model's delay", () => {
    const loop = smithLoop();
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    assert.throws(() => projectControllerSeed(loop, model, { ...task, settlingTimeSeconds: 0.8, durationSeconds: 5 }, loop.controller.project.seed),
      /delay/);
  });

  test("Tune names the PI gains and the program's model values", () => {
    const loop = smithLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "Smith", values: { "P1.Parameters.ProportionalGain": 10,
        "P1.Parameters.IntegralGain": 2, "P1.Parameters.DerivativeGain": 0, "P1.ModelGain": 0.2, "P1.ModelTimeConstantS": 5, "P1.ModelDelayS": 1 } });
      assert.deepStrictEqual([...dom.window.document.querySelectorAll("dt")].map((label: any) => label.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)", "Model gain K", "Model time constant T", "Model delay θ"]);
    } finally { (globalThis as any).document = saved; }
  });
});

suite("Control Studio Smith predictor setup", () => {
  const formModel = {
    structure: "pid", addressedReal: ["P1.HeaterPct", "P1.ManualPct", "P1.SetpointC", "P1.WaterC"], booleans: ["P1.Auto"], modes: ["P1.Auto"],
    records: ["P1.Parameters"], writes: [], initializes: [], pids: [{ instance: "P1.Loop", task: { name: "MainTask", intervalMs: 100 } }], instances: ["P1"],
    reals: ["P1.CorrectionC", "P1.HeaterPct", "P1.ManualPct", "P1.ModelDelayS", "P1.ModelGain", "P1.ModelTimeConstantS", "P1.SetpointC", "P1.WaterC"],
    prefill: { instance: "P1.Loop", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.HeaterPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualPct" },
  };
  test("sets up the PID and the program's predictor variables in one form", () => {
    const { open } = require("./control-studio-loop-setup-form-harness");
    const view = open(formModel);
    try {
      const structures = Array.from(view.root.querySelectorAll('[data-setup-field="structure"] option') as ArrayLike<{ value: string }>).map(option => option.value);
      assert.ok(structures.includes("smith"), `a Smith predictor has its own structure: ${structures}`);
      view.set("structure", "smith");
      for (const [field, value] of [["name", "Water temperature"], ["processVariable", "P1.WaterC"], ["processVariable.unit", "degC"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "80"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"], ["mode", "P1.Auto"], ["experimentDuration", "60"]]) view.set(field, value);
      const save = view.root.querySelector("[data-setup-save]"), message = view.root.querySelector("[data-setup-message]");
      assert.strictEqual(save.disabled, true);
      assert.match(message.textContent ?? "", /model gain/i, "the predictor's variables are next");
      for (const [field, value] of [["smith.gain", "P1.ModelGain"], ["smith.timeConstant", "P1.ModelTimeConstantS"], ["smith.delay", "P1.ModelDelayS"],
        ["smith.correction", "P1.CorrectionC"]]) view.set(field, value);
      assert.strictEqual(save.disabled, false, message.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts, [{ kind: "control-studio.setup.save", draft: {
        structure: "smith", name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
        processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: 0, maximum: 80 },
        manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
        setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct", experimentStep: 10,
        experimentDurationSeconds: 60,
        model: { gain: "P1.ModelGain", timeConstant: "P1.ModelTimeConstantS", delay: "P1.ModelDelayS", correction: "P1.CorrectionC" },
      } }]);
    } finally { view.close(); }
  });

  test("saves the Smith predictor loop from the confirmed draft", async () => {
    const fs = require("fs"), os = require("os"), path = require("path");
    const { saveLoopSetup } = require("../../controlStudio/loopSetup/loopSetupSave");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-smith-setup-"));
    try {
      const { projectId: _project, runtimeId: _runtime, ...unscoped } = draft();
      const requests = await saveLoopSetup(root, scope, heater, { structure: "smith", ...unscoped });
      assert.deepStrictEqual(requests.map((request: any) => request.loopDefinition.controller.project?.family), ["Smith"]);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
