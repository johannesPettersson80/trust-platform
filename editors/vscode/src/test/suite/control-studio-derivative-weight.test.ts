import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { decodePidLaw } from "../../controlStudio/controllerLaw";
import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { manualControllerCandidate } from "../../controlStudio/manualControllerCandidate";
import { engineerPrediction } from "../../controlStudio/engineerPrediction";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
const LESSON = "examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping";
const WEIGHT = "P1.Parameters.DerivativeSetpointWeight";
const KP = "P1.Parameters.ProportionalGain", KD = "P1.Parameters.DerivativeGain";

/** Lesson 34's PD position loop, whose law declares the derivative setpoint weight γ, and its shaft model. */
function lessonFile(name: string): any {
  const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  return JSON.parse(fs.readFileSync(path.join(root, LESSON, name), "utf8"));
}

suite("Control Studio derivative setpoint weight", () => {
  test("declares the derivative setpoint weight as a Control.PID record field between 0 and 1", () => {
    const loop = lessonFile("control-loop.json");
    assert.strictEqual(decodePidLaw(loop.controller.law, loop.parameterBinding)?.derivativeSetpointWeightSymbol, WEIGHT);
    const binding = (change: Record<string, unknown>) => ({ ...loop.parameterBinding, parameters: loop.parameterBinding.parameters
      .map((parameter: any) => parameter.symbol === WEIGHT ? { ...parameter, ...change } : parameter) });
    for (const [label, change] of [["a weight is dimensionless", { unit: "%" }], ["a weight is at most 1", { maximum: 2 }],
      ["a weight is at least 0", { minimum: -1 }], ["the weight is a declared parameter", { symbol: "P1.Other" }]] as const) {
      assert.strictEqual(decodePidLaw(loop.controller.law, binding(change)), undefined, label);
    }
  });

  test("writes the entered derivative weight and a calculated candidate differentiates the measurement", () => {
    const loop = lessonFile("control-loop.json"), model = lessonFile("shaft-model.json");
    const tuned = manualControllerCandidate(loop, model, "PD", { [KP]: 16, [KD]: 7, [WEIGHT]: 1 });
    assert.strictEqual(tuned.values[WEIGHT], 1, "the ST record receives the engineer's γ");
    assert.strictEqual((tuned.design.candidate as any).derivativeSetpointWeight, 1, "the prediction uses the same γ");
    assert.throws(() => manualControllerCandidate(loop, model, "PD", { [KP]: 16, [KD]: 7 }), /Enter every required/,
      "a declared derivative weight is part of the manual PD settings");
    assert.strictEqual(controllerAdapter(loop).materialize({ proportionalGain: 16, integralGain: 0, derivativeGain: 7 }, {})[WEIGHT], 0,
      "a calculated candidate differentiates the measurement, so its record states γ = 0");
  });

  test("offers the derivative weight beside the PD gains", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const root = dom.window.document.querySelector("main");
      const controls = initializeManualTuneSettings(root, lessonFile("control-loop.json"), undefined);
      controls.render({ status: "candidate", family: "PD", values: { [KP]: 16, [KD]: 7, [WEIGHT]: 0 } });
      const field = root.querySelector(`[data-studio-manual-value='${WEIGHT}']`);
      assert.ok(field, "PD shows the declared derivative setpoint weight");
      assert.strictEqual(root.querySelector(`label[for='${field.id}']`).textContent, "Derivative setpoint weight (γ)");
      assert.strictEqual(field.nextElementSibling.textContent, "", "a dimensionless weight shows no unit");
      assert.strictEqual(controls.read().values[WEIGHT], 0);
      controls.render({ status: "candidate", family: "P", values: { [KP]: 16 } });
      assert.strictEqual(root.querySelector(`[data-studio-manual-value='${WEIGHT}']`), null, "P has no derivative term to weight");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("predicts the derivative kick of a target step", () => {
    const task = { referenceChange: 1, maximumError: 0.02, settlingBand: 0.01, settlingTimeSeconds: 4, durationSeconds: 8 };
    const firstTorque = (weight: number) => {
      const plots = engineerPrediction(lessonFile("shaft-model.json"), lessonFile("control-loop.json"),
        { proportionalGain: 16, derivativeGain: 7, derivativeSetpointWeight: weight } as any, task);
      assert.ok(plots, `γ = ${weight} must be predicted`);
      return plots!.timeResponse[1].candidateMv;
    };
    assert.strictEqual(firstTorque(0), 16, "D on the measurement: only Kp · 1 rad at the step");
    assert.strictEqual(firstTorque(1), 20, "D on the error: 16 + 7 · 1/0.02 N·m, clipped at the 20 N·m limit");
  });
});
