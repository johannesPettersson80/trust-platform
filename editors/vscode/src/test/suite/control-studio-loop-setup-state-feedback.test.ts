import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** Loaded dynamically so that a missing state-feedback generator fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 1000 }];
const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** Two tanks in series: the program feeds both measured levels back to the pump. */
const tanks = {
  symbols: [
    { symbol: "P1.Tank1M", type: "LREAL", address: "%IL0" }, { symbol: "P1.Tank2M", type: "LREAL", address: "%IL8" },
    { symbol: "P1.PumpM3PerS", type: "LREAL", address: "%QL0" }, { symbol: "P1.Tank2SetpointM", type: "LREAL", address: "%ML0" },
    { symbol: "P1.ManualM3PerS", type: "LREAL", address: "%ML8" }, { symbol: "P1.FeedbackEnabled", type: "BOOL" },
    { symbol: "P1.K1", type: "LREAL" }, { symbol: "P1.K2", type: "LREAL" }, { symbol: "P1.N", type: "LREAL" },
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }],
  links: [],
  writes: ["P1.PumpM3PerS"],
};
const draft = { ...scope, name: "Tank 2 level", action: "direct", family: "LQR", instance: "P1",
  manipulatedVariable: { symbol: "P1.PumpM3PerS", unit: "m3/s", minimum: 0, maximum: 0.02 },
  processVariable: { symbol: "P1.Tank2M", unit: "m", minimum: 0, maximum: 0.5 }, setpoint: { symbol: "P1.Tank2SetpointM" },
  gain: "P1.K2", referenceGain: "P1.N", states: [{ signal: { symbol: "P1.Tank1M", unit: "m", minimum: 0, maximum: 0.5 }, gain: "P1.K1" }],
  mode: { symbol: "P1.FeedbackEnabled", whenTrue: "Auto" }, manualOutput: "P1.ManualM3PerS", experimentStep: 0.002, experimentDurationSeconds: 300 };

/** The two tanks linearized at 0.25 m and sampled at 0.1 s, in the measured coordinates (tank 2, tank 1). */
function tankModel() {
  const channel = (id: string, unit: string, minimum: number, maximum: number) => ({ id, unit, operatingRange: { minimum, maximum } });
  return { kind: "discrete-state-space", sampleTimeSeconds: 1, revision: "tanks", source: "fitted", validation: "validated", evidence: ["recording"],
    delaySeconds: 0, inputs: [channel("P1.PumpM3PerS", "m3/s", 0, 0.02)], outputs: [channel("P1.Tank2M", "m", 0, 0.5), channel("P1.Tank1M", "m", 0, 0.5)],
    // As identification names them: the states are the measured outputs because C selects each one, not by their ids.
    states: [channel("history-0", "m", -0.25, 0.25), channel("history-1", "m", -0.25, 0.25)],
    a: [[0.96, 0.04], [0, 0.96]], b: [[0], [2]], c: [[1, 0], [0, 1]], d: [[0], [0]], operatingPoint: { inputs: [0.01], outputs: [0.25, 0.25] } };
}

suite("Control Studio state-feedback setup", () => {
  test("allows the law's response to the deviation Apply accepts as settled at Auto entry", () => {
    const api = setup();
    const loop = api.stateFeedbackLoopDefinition(tanks, draft);
    const { autoEntryTolerance } = require("../../controlStudio/commissioningStartingPoint");
    assert.strictEqual(typeof autoEntryTolerance, "function", "Apply needs the Auto-entry tolerance of the controller it enters");
    const task = { referenceChange: 0.01, maximumError: 0.003, settlingBand: 0.001, settlingTimeSeconds: 150, durationSeconds: 300 };
    const values = { "P1.K2": 0.0106, "P1.K1": 0.0291, "P1.N": 0.0598 };
    const allowed = autoEntryTolerance(loop, tankModel(), task, values);
    assert.ok(Math.abs(allowed - (0.0106 + 0.0291 + 0.0598) * 0.001) < 1e-12,
      `each gain times the settling band of its output, the reference following the measurement: ${allowed}`);
    assert.strictEqual(autoEntryTolerance(loop, tankModel(), { ...task, transferTolerance: 0.0002 }, values), 0.0002, "a reviewed tolerance replaces the allowance");
    const lagged = { ...tankModel(), c: [[1, 0], [0, 0]] };
    assert.strictEqual(autoEntryTolerance(loop, lagged, task, values), require("../../controlStudio/proportionalTransfer").transferTolerance(loop),
      "a state that is not a measured output has no accepted deviation, so the exact check stays");
    const onOff = { ...loop, controller: { ...loop.controller, stateFeedback: undefined } };
    const { transferTolerance } = require("../../controlStudio/proportionalTransfer");
    assert.strictEqual(autoEntryTolerance(onOff, tankModel(), task, values), transferTolerance(onOff), "a controller with bumpless states keeps the exact check");
  });

  test("shows each designed gain with the exact symbol it belongs to", () => {
    const api = setup();
    const loop = api.stateFeedbackLoopDefinition(tanks, draft);
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<main><div data-studio-tuning-result hidden></div></main>");
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      const root = dom.window.document.querySelector("main");
      renderTunedController(root, loop, { status: "candidate", family: "LQR", values: { "P1.K2": 0.0106, "P1.K1": 0.0291, "P1.N": 0.0598 } });
      const labels = [...root.querySelectorAll("dt")].map((label: any) => label.textContent);
      assert.deepStrictEqual(labels, ["State feedback gain on P1.Tank2M", "State feedback gain on P1.Tank1M", "Reference gain on P1.Tank2SetpointM"],
        "a symbol is shown exactly as the program declares it");
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });

  test("writes a state-feedback loop whose gains belong to measured outputs and are designed on the identified model", () => {
    const api = setup();
    assert.strictEqual(typeof api.stateFeedbackLoopDefinition, "function", "state feedback needs its own setup structure");
    const loop = api.stateFeedbackLoopDefinition(tanks, draft);
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual(loop.channels.outputs.map((output: any) => [output.modelChannelId, output.signalId, output.setpointSignalId]),
      [["P1.Tank2M", "processVariable", "setpoint"], ["P1.Tank1M", "state-2", undefined]]);
    assert.deepStrictEqual([loop.controller.stateFeedback.family, loop.controller.stateFeedback.gainSymbols, loop.controller.stateFeedback.referenceGainSymbols],
      ["LQR", [["P1.K2", "P1.K1"]], [["P1.N"]]], "one gain per measured output, in the model's state order");
    assert.strictEqual(loop.identificationPolicy.maximumOrder, 1, "each model state is one measured output");
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit]),
      [["P1.K2", "m3/s/m"], ["P1.K1", "m3/s/m"], ["P1.N", "m3/s/m"]]);
    assert.deepStrictEqual([loop.experiment.kind, loop.experiment.amplitude, loop.experiment.durationSeconds], ["mv_step", 0.002, 300]);
    // The two tanks linearized at 0.25 m and sampled at the task period, in the measured coordinates (tank 2, tank 1).
    const channel = (id: string, unit: string, maximum: number) => ({ id, unit, operatingRange: { minimum: 0, maximum } });
    const model = { kind: "discrete-state-space", sampleTimeSeconds: 1, revision: "tanks", source: "fitted", validation: "validated", evidence: ["recording"],
      delaySeconds: 0, inputs: [channel("P1.PumpM3PerS", "m3/s", 0.02)], outputs: [channel("P1.Tank2M", "m", 0.5), channel("P1.Tank1M", "m", 0.5)],
      states: [channel("P1.Tank2M", "m", 0.5), channel("P1.Tank1M", "m", 0.5)],
      a: [[0.96, 0.04], [0, 0.96]], b: [[0], [2]], c: [[1, 0], [0, 1]], d: [[0], [0]], operatingPoint: { inputs: [0.01], outputs: [0.25, 0.25] } };
    const { designStateController } = require("../../controlStudio/stateControllerDesign");
    const design = designStateController(loop, model, 60);
    assert.deepStrictEqual(Object.keys(design.candidate.parameters).sort(), ["P1.K1", "P1.K2", "P1.N"]);
    assert.ok(design.closedLoopPoles.every((pole: any) => Math.hypot(pole.real, pole.imaginary) < 1), "the designed gains stabilize the model");
  });
});

suite("Control Studio state-feedback margins", () => {
  test("shows the margins of the loop broken at the plant input, L = K(zI − A)⁻¹B on the sampled model", () => {
    const api = setup();
    const loop = api.stateFeedbackLoopDefinition(tanks, draft);
    let margins: any;
    try { margins = require("../../controlStudio/candidateMargins").candidateMargins(tankModel(), loop, { candidate: { parameters: { "P1.K2": 0.05, "P1.K1": 0.1, "P1.N": 0.1 } } }, undefined, undefined); }
    catch (error) { margins = String(error); }
    // Independent oracle: with p = z − 0.96, L(z) = 0.004/p² + 0.2/p; bisect |L(e^{jω})| = 1 and read its phase.
    const loopAt = (omega: number) => {
      const re = Math.cos(omega) - 0.96, im = Math.sin(omega);
      const inv = { re: re / (re * re + im * im), im: -im / (re * re + im * im) };
      const inv2 = { re: inv.re * inv.re - inv.im * inv.im, im: 2 * inv.re * inv.im };
      return { re: 0.004 * inv2.re + 0.2 * inv.re, im: 0.004 * inv2.im + 0.2 * inv.im };
    };
    let low = 1e-4, high = Math.PI * 0.999;
    for (let step = 0; step < 100; step++) { const middle = (low + high) / 2; const value = loopAt(middle); if (Math.hypot(value.re, value.im) > 1) low = middle; else high = middle; }
    const crossing = loopAt(low), phaseMargin = 180 + Math.atan2(crossing.im, crossing.re) * 180 / Math.PI;
    assert.ok(margins && typeof margins.phaseMarginDegrees === "number", `margins: ${JSON.stringify(margins)}`);
    assert.ok(Math.abs(margins.phaseMarginDegrees - phaseMargin) < 0.5, `${margins.phaseMarginDegrees}° against ${phaseMargin}°`);
    assert.ok(Math.abs(margins.delayMarginSeconds - phaseMargin * Math.PI / 180 / low) < 0.05 * phaseMargin * Math.PI / 180 / low, JSON.stringify(margins));
    assert.ok(margins.peakSensitivity >= 1, JSON.stringify(margins));
  });
});
