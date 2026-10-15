import * as assert from "assert";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { cascade, cascadeTogetherDraft as jointDraft, scope } from "./control-studio-cascade-fixtures";

/** Loaded dynamically so that a missing generator fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

/** Jacket 2 s and 0.2 °C/% from the heater; the vessel follows the jacket with 10 s, both measured (Specification 35 §17.11). */
const model: any = { kind: "continuous-state-space", revision: "vessel", source: "fitted", validation: "validated", evidence: ["recording"], delaySeconds: 0,
  a: [[-0.5, 0], [0.1, -0.1]], b: [[0.1], [0]], c: [[1, 0], [0, 1]], d: [[0], [0]],
  states: [{ id: "P1.JacketC", unit: "degC", operatingRange: { minimum: -40, maximum: 50 } }, { id: "P1.VesselC", unit: "degC", operatingRange: { minimum: -40, maximum: 50 } }],
  inputs: [{ id: "P1.HeaterPct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "P1.JacketC", unit: "degC", operatingRange: { minimum: 0, maximum: 90 } }, { id: "P1.VesselC", unit: "degC", operatingRange: { minimum: 0, maximum: 90 } }],
  operatingPoint: { inputs: [50], outputs: [40, 40] } };
const task = { referenceChange: 2, maximumError: 0.5, settlingBand: 0.2, settlingTimeSeconds: 60, durationSeconds: 120 };

suite("Control Studio cascade tuned together", () => {
  test("writes one cascade loop from the plant input to both measurements, with both controllers switched together", () => {
    const api = setup();
    assert.strictEqual(typeof api.cascadeLoopDefinition, "function", "a cascade tuned together needs one loop for both controllers");
    const loop = api.cascadeLoopDefinition(cascade, jointDraft());
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual(loop.channels.inputs.map((input: any) => input.modelChannelId), ["P1.HeaterPct"]);
    assert.deepStrictEqual(loop.channels.outputs.map((output: any) => [output.modelChannelId, output.setpointSignalId]),
      [["P1.JacketC", undefined], ["P1.VesselC", "setpoint"]], "the plant input moves both measurements; only the outer one has a setpoint");
    assert.strictEqual(loop.identificationPolicy.maximumOrder, 1, "first order in each measurement, so the vessel follows the jacket");
    const project = loop.controller.project;
    assert.deepStrictEqual([project.family, project.seed.kind, project.additionalModes],
      ["cascade", "cascade-pid", [{ symbol: "P1.OuterAuto", manualValue: false }]]);
    assert.deepStrictEqual([loop.controller.modeSignal, loop.parameterBinding.mode.symbol, loop.controller.manualOutputSignal],
      ["P1.InnerAuto", "P1.InnerAuto", "P1.InnerManual"], "identification steps the inner controller's manual output");
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit]), [
      ["P1.InnerParameters.ProportionalGain", "%/degC"], ["P1.InnerParameters.IntegralGain", "%/(degC*s)"], ["P1.InnerParameters.DerivativeGain", "%*s/degC"],
      ["P1.OuterParameters.ProportionalGain", "degC/degC"], ["P1.OuterParameters.IntegralGain", "degC/(degC*s)"], ["P1.OuterParameters.DerivativeGain", "degC*s/degC"]]);
    const initial = (symbol: string) => project.initialization.states.find((state: any) => state.symbol === symbol)?.initial;
    assert.deepStrictEqual([initial("P1.Inner.StateValue.NominalOutput"), initial("P1.Inner.StateValue.Setpoint")],
      [{ source: "model-input", index: 0 }, { source: "model-output", index: 0 }], "the inner controller starts at the plant input, its setpoint at the jacket");
    assert.deepStrictEqual([initial("P1.Outer.StateValue.NominalOutput"), initial("P1.Outer.StateValue.ProcessValue")],
      [{ source: "model-output", index: 0 }, { source: "model-output", index: 1 }], "the outer output starts at the jacket measurement, so no switch bumps");
    assert.strictEqual(project.objective.requirementId, "P1.VesselC:target-error");
    assert.deepStrictEqual([loop.experiment.kind, loop.experiment.amplitude, loop.experiment.durationSeconds], ["mv_step", 10, 120]);
  });

  test("switches the outer controller's mode in the same transfer as the inner one", () => {
    const api = setup();
    assert.strictEqual(typeof api.cascadeLoopDefinition, "function", "a cascade tuned together needs one loop for both controllers");
    const loop = api.cascadeLoopDefinition(cascade, jointDraft());
    const { declaredTransferBinding } = require("../../controlStudio/controllerInitialization");
    const binding = declaredTransferBinding(loop, loop.controller.project.initialization);
    assert.deepStrictEqual([binding.mode, binding.additional_modes], [{ symbol: "P1.InnerAuto", manual_value: false },
      [{ symbol: "P1.OuterAuto", manual_value: false }]]);
  });

  test("validates with both controllers in Auto", () => {
    const api = setup();
    assert.strictEqual(typeof api.cascadeLoopDefinition, "function", "a cascade tuned together needs one loop for both controllers");
    const loop = api.cascadeLoopDefinition(cascade, jointDraft());
    const { engineerVerificationCampaign } = require("../../controlStudio/engineerVerificationCampaign");
    const values = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.symbol.endsWith("DerivativeGain") ? 0 : 1]));
    for (const scenario of engineerVerificationCampaign(loop, model, task, values)) {
      assert.deepStrictEqual([scenario.request.initial_values["P1.InnerAuto"], scenario.request.initial_values["P1.OuterAuto"]], [1, 1],
        `${scenario.id}: the outer controller sets the inner setpoint only in Auto`);
    }
  });

  test("Tune names each controller's gains and leaves out the unused derivative gains", () => {
    const api = setup();
    assert.strictEqual(typeof api.cascadeLoopDefinition, "function", "a cascade tuned together needs one loop for both controllers");
    const loop = api.cascadeLoopDefinition(cascade, jointDraft());
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      const values = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.symbol.endsWith("DerivativeGain") ? 0 : 1]));
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "cascade", values });
      // The served cascade shows one row for each controller, inner first, with its instance and its P and I (structureResults.ts).
      const table = dom.window.document.querySelector("[data-studio-cascade-design]");
      assert.ok(table, "the cascade's controllers are shown as one table");
      assert.deepStrictEqual([...table.tBodies[0].rows].map((row: any) => row.cells[0].textContent), [
        `Inner · ${loop.controller.project.seed.inner.instance}`, `Outer · ${loop.controller.project.seed.outer.instance}`]);
      assert.strictEqual(dom.window.document.querySelectorAll("dt").length, 0, "the derivative gains are left out");
    } finally { (globalThis as any).document = saved; }
  });

  test("designs the inner loop first and the outer loop through the closed inner loop, meeting the outer requirements", () => {
    const api = setup();
    assert.strictEqual(typeof api.cascadeLoopDefinition, "function", "a cascade tuned together needs one loop for both controllers");
    const loop = api.cascadeLoopDefinition(cascade, jointDraft());
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const values = projectControllerSeed(loop, model, task, loop.controller.project.seed);
    const inner = { kp: values["P1.InnerParameters.ProportionalGain"], ki: values["P1.InnerParameters.IntegralGain"] };
    const outer = { kp: values["P1.OuterParameters.ProportionalGain"], ki: values["P1.OuterParameters.IntegralGain"] };
    assert.ok([inner.kp, inner.ki, outer.kp, outer.ki].every(value => Number.isFinite(value) && value > 0), JSON.stringify(values));
    assert.deepStrictEqual([values["P1.InnerParameters.DerivativeGain"], values["P1.OuterParameters.DerivativeGain"]], [0, 0]);
    // Both PIs on the model, the outer output setting the jacket setpoint: the vessel must settle by 60 s within 0.5 °C.
    let jacket = 0, vessel = 0, innerIntegral = 0, outerIntegral = 0, error = 0;
    const h = 0.01, target = task.referenceChange;
    for (let step = 0; step * h < task.durationSeconds; step++) {
      const outerError = target - vessel;
      const jacketSetpoint = outer.kp * outerError + outerIntegral;
      const innerError = jacketSetpoint - jacket;
      const heater = inner.kp * innerError + innerIntegral;
      outerIntegral += outer.ki * outerError * h; innerIntegral += inner.ki * innerError * h;
      jacket += h * (-0.5 * jacket + 0.1 * heater); vessel += h * (0.1 * jacket - 0.1 * vessel);
      if (step * h >= task.settlingTimeSeconds) error = Math.max(error, Math.abs(vessel - target));
    }
    assert.ok(error <= task.maximumError, `the cascade is ${error} °C from the target after 60 s; 0.5 °C is allowed`);
    assert.ok(inner.ki / inner.kp > outer.ki / outer.kp, "the inner loop integrates faster than the outer one");
  });
});
