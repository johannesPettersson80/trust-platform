import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { decodePidLaw } from "../../controlStudio/controllerLaw";
import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { manualControllerCandidate } from "../../controlStudio/manualControllerCandidate";
import { engineerPrediction } from "../../controlStudio/engineerPrediction";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
const WEIGHT = "P1.Parameters.ProportionalSetpointWeight";

/** The heater's Control.PID loop whose law also declares the proportional setpoint weight β. */
function weightedLoop(): any {
  const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  const loop = JSON.parse(fs.readFileSync(path.join(root,
    "examples/control/06-pid-tuning-methods/31-tuning-method-comparison/control-loop.json"), "utf8"));
  loop.controller.law = { ...loop.controller.law, proportionalSetpointWeightSymbol: WEIGHT };
  return loop;
}

function heaterModel(): any {
  const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 100 } });
  return { kind: "transfer-function", numerator: [0.2], denominator: [5, 1], delaySeconds: 1, revision: "heater", source: "fitted",
    validation: "validated", evidence: ["step"], inputs: [channel("P1.AppliedPowerPct", "%")], outputs: [channel("P1.TemperatureC", "degC")],
    operatingPoint: { inputs: [75], outputs: [35] } };
}

suite("Control Studio setpoint weight", () => {
  test("declares the setpoint weight as a Control.PID record field between 0 and 1", () => {
    const loop = weightedLoop();
    assert.strictEqual(decodePidLaw(loop.controller.law, loop.parameterBinding)?.proportionalSetpointWeightSymbol, WEIGHT);
    const binding = (change: Record<string, unknown>) => ({ ...loop.parameterBinding, parameters: loop.parameterBinding.parameters
      .map((parameter: any) => parameter.symbol === WEIGHT ? { ...parameter, ...change } : parameter) });
    for (const [label, change] of [["a weight is dimensionless", { unit: "%" }], ["a weight is at most 1", { maximum: 2 }],
      ["a weight is at least 0", { minimum: -1 }], ["the weight is a declared parameter", { symbol: "P1.Other" }]] as const) {
      assert.strictEqual(decodePidLaw(loop.controller.law, binding(change)), undefined, label);
    }
  });

  test("writes the entered setpoint weight into the Control.PID record and standard PI keeps β = 1", () => {
    const loop = weightedLoop();
    const tuned = manualControllerCandidate(loop, heaterModel(), "PI",
      { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5, [WEIGHT]: 0.5 });
    assert.strictEqual(tuned.values[WEIGHT], 0.5, "the ST record receives the engineer's β");
    assert.strictEqual((tuned.design.candidate as any).proportionalSetpointWeight, 0.5, "the prediction uses the same β");
    assert.throws(() => manualControllerCandidate(loop, heaterModel(), "PI",
      { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5 }), /Enter every required/,
      "a declared weight is part of the manual PI settings");
    assert.strictEqual(controllerAdapter(loop).materialize({ proportionalGain: 13, integralGain: 3.5, derivativeGain: 0 }, {})[WEIGHT], 1,
      "a calculated PI candidate is the standard form, so its record states β = 1");
  });

  test("offers the setpoint weight beside the PI gains", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const root = dom.window.document.querySelector("main");
      const controls = initializeManualTuneSettings(root, weightedLoop(), undefined);
      controls.render({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5, [WEIGHT]: 1 } });
      const field = root.querySelector(`[data-studio-manual-value='${WEIGHT}']`);
      assert.ok(field, "PI shows the declared setpoint weight");
      assert.strictEqual(root.querySelector(`label[for='${field.id}']`).textContent, "Setpoint weight (β)");
      assert.strictEqual(field.nextElementSibling.textContent, "", "a dimensionless weight shows no unit, not a stray 1");
      assert.strictEqual(controls.read().values[WEIGHT], 1);
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("predicts the smaller setpoint kick of a weighted proportional term", () => {
    const task = { referenceChange: 3, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 30, durationSeconds: 60 };
    const jump = (weight?: number) => {
      const plots: any = engineerPrediction(heaterModel(), weightedLoop(), { proportionalGain: 2, integralGain: 0.5,
        ...(weight === undefined ? {} : { proportionalSetpointWeight: weight }) } as any, task);
      const response = plots?.timeResponse ?? plots?.plots?.timeResponse;
      const first = response.find((point: any) => Math.abs(point.candidateMv - response[0].candidateMv) > 1e-9);
      return first.candidateMv - response[0].candidateMv;
    };
    const full = jump(), half = jump(0.5);
    assert.ok(Math.abs(full - 6.15) < 0.05, `β = 1: Kp·3 °C + Ki·3·0.1 = 6.15 points, got ${full}`);
    assert.ok(Math.abs(half - 3.15) < 0.05, `β = 0.5: 2·0.5·3 + 0.15 = 3.15 points, got ${half}`);
  });
});
