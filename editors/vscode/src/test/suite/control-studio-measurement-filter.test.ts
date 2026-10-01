import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { decodePidLaw } from "../../controlStudio/controllerLaw";
import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { manualControllerCandidate } from "../../controlStudio/manualControllerCandidate";
import { engineerPrediction } from "../../controlStudio/engineerPrediction";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
const FILTER = "P1.FilterTimeS";

/** The heater's Control.PID loop whose program filters the measurement with a declared filter time. */
function filteredLoop(): any {
  const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  const loop = JSON.parse(fs.readFileSync(path.join(root,
    "examples/control/06-pid-tuning-methods/31-tuning-method-comparison/control-loop.json"), "utf8"));
  loop.controller.law = { ...loop.controller.law, measurementFilterTimeSymbol: FILTER };
  loop.parameterBinding.parameters.push({ symbol: FILTER, iec_type: "LREAL", unit: "s", convention: "measurement filter time", minimum: 0, maximum: 5 });
  return loop;
}

function heaterModel(): any {
  const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 100 } });
  return { kind: "transfer-function", numerator: [0.2], denominator: [5, 1], delaySeconds: 1, revision: "heater", source: "fitted",
    validation: "validated", evidence: ["step"], inputs: [channel("P1.AppliedPowerPct", "%")], outputs: [channel("P1.TemperatureC", "degC")],
    operatingPoint: { inputs: [75], outputs: [35] } };
}

suite("Control Studio measurement filter", () => {
  test("declares the measurement filter time as a bound setting in seconds", () => {
    const loop = filteredLoop();
    assert.strictEqual(decodePidLaw(loop.controller.law, loop.parameterBinding)?.measurementFilterTimeSymbol, FILTER);
    const binding = (change: Record<string, unknown>) => ({ ...loop.parameterBinding, parameters: loop.parameterBinding.parameters
      .map((parameter: any) => parameter.symbol === FILTER ? { ...parameter, ...change } : parameter) });
    for (const [label, change] of [["a filter time is in seconds", { unit: "1" }], ["a filter time is not negative", { minimum: -1 }],
      ["the filter time is a declared parameter", { symbol: "P1.Other" }]] as const) {
      assert.strictEqual(decodePidLaw(loop.controller.law, binding(change)), undefined, label);
    }
  });

  test("writes the entered filter time and a calculated candidate bypasses the filter", () => {
    const loop = filteredLoop();
    const gains = { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5 };
    const tuned = manualControllerCandidate(loop, heaterModel(), "PI", { ...gains, [FILTER]: 1 });
    assert.strictEqual(tuned.values[FILTER], 1, "the program receives the engineer's Tf");
    assert.strictEqual((tuned.design.candidate as any).measurementFilterSeconds, 1, "the prediction uses the same Tf");
    assert.throws(() => manualControllerCandidate(loop, heaterModel(), "PI", gains), /Enter every required/,
      "a declared filter time is part of the manual settings");
    assert.strictEqual(controllerAdapter(loop).materialize({ proportionalGain: 13, integralGain: 3.5, derivativeGain: 0 }, {})[FILTER], 0,
      "a calculated candidate is designed without a filter, so Tf = 0 bypasses it");
  });

  test("offers the filter time beside the controller gains", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const root = dom.window.document.querySelector("main");
      const controls = initializeManualTuneSettings(root, filteredLoop(), undefined);
      controls.render({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5, [FILTER]: 0 } });
      const field = root.querySelector(`[data-studio-manual-value='${FILTER}']`);
      assert.ok(field, "PI shows the declared measurement filter time");
      assert.strictEqual(root.querySelector(`label[for='${field.id}']`).textContent, "Measurement filter time (Tf)");
      assert.strictEqual(field.nextElementSibling.textContent, "s");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("predicts the filter's lag in the closed loop", () => {
    const task = { referenceChange: 3, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 30, durationSeconds: 60 };
    const peak = (filter?: number) => {
      const plots: any = engineerPrediction(heaterModel(), filteredLoop(), { proportionalGain: 13, integralGain: 3.5,
        ...(filter === undefined ? {} : { measurementFilterSeconds: filter }) } as any, task);
      return Math.max(...plots.timeResponse.map((point: any) => point.candidatePv));
    };
    const bypass = peak(), lagged = peak(1);
    assert.ok(lagged > bypass + 0.05, `a 1 s filter delays the feedback and raises the predicted peak: ${bypass} → ${lagged}`);
    assert.strictEqual(peak(0), bypass, "Tf = 0 is the bypass");
  });
});
