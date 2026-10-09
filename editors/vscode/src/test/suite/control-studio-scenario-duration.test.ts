import * as assert from "assert";
import { mountThreePageScenarios } from "../../controlStudio/webview/threePageScenarios";
import { engineerVerificationCampaign } from "../../controlStudio/engineerVerificationCampaign";
import { engineerRequirements, engineerVerificationPlan } from "../../controlStudio/engineerVerificationPlan";
import { assertRuntimeRecordingBounds, isolatedRecordEvery } from "../../controlStudio/verificationClient";
import { commissioningObservation, validCommissioningObservation } from "../../controlStudio/commissioningObservation";
import { classifyMainWorkflowFailure } from "../../controlStudio/mainWorkflowFailure";
import type { GeneralModel } from "../../controlStudio/generalModel";
import { channel, pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** A delayed-update scenario: an event 1 s into the run. */
const scenario = { id: "delayed-update", initialValues: { "P1.SetpointC": 35 },
  events: [{ atMs: 0, symbol: "P1.DisturbanceKw", value: 30 }, { atMs: 1000, symbol: "P1.DelayUpdateS", value: 2 }],
  requirements: [{ id: "delayed-minimum", kind: "bounds", channel: "P1.TemperatureC", window: { start: 0, end: 60 }, minimum: 33, maximum: 80 }] };

suite("Control Studio scenario timing", () => {
  test("limits scenario event times by the validation duration entered in the same dialog", () => {
    const dom = new JSDOM(`<main><section data-trust-compact-editor><form>
      <input data-studio-requirement="durationSeconds" type="number" value=""><div class="trust-compact-editor-actions"></div></form></section></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const dialog = dom.window.document.querySelector("[data-trust-compact-editor]");
      const editor = mountThreePageScenarios(dialog, { scenarioDefinitions: [scenario], processSymbol: "P1.TemperatureC",
        setpointSymbol: "P1.SetpointC", samplePeriodMs: 100, durationSeconds: 0 });
      assert.strictEqual(editor.validate(), undefined, "an event at 1 s is not limited before a duration is entered");
      const duration = dialog.querySelector("[data-studio-requirement='durationSeconds']");
      duration.value = "60"; duration.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.strictEqual(editor.validate(), undefined, "an event at 1 s lies inside a 60 s validation");
      duration.value = "0.5"; duration.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.ok(editor.validate(), "an event after the end of a 0.5 s validation is rejected");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("limits scenario requirement windows by the validation duration entered in the same dialog", () => {
    const dom = new JSDOM(`<main><section data-trust-compact-editor><form>
      <input data-studio-requirement="durationSeconds" type="number" value="60"><div class="trust-compact-editor-actions"></div></form></section></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const dialog = dom.window.document.querySelector("[data-trust-compact-editor]");
      const editor = mountThreePageScenarios(dialog, { scenarioDefinitions: [scenario], processSymbol: "P1.TemperatureC",
        setpointSymbol: "P1.SetpointC", samplePeriodMs: 100, durationSeconds: 60 });
      assert.strictEqual(editor.validate(), undefined, "a check until 60 s lies inside a 60 s validation");
      const duration = dialog.querySelector("[data-studio-requirement='durationSeconds']");
      duration.value = "30"; duration.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.match(editor.validate() ?? "", /delayed-update.+until 60 s.+at least 60 s/,
        "a check until 60 s cannot be evaluated in a 30 s validation; the dialog names the duration it needs");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("simulates a long validation at the task period and records it at a record period that follows the process (A4.2, A7)", () => {
    // A slow process: 4 h of validation at a 100 ms task is 144,001 scans, more than one run keeps without a record period.
    const loop = pidLoop();
    loop.controller.taskPeriodMs = 100;
    const model: GeneralModel = { kind: "transfer-function", numerator: [1.5], denominator: [4500, 1], delaySeconds: 10,
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] },
      source: "provided", validation: "validated", evidence: ["declared-fixture"], revision: "slow-process" };
    const task = engineerRequirements({ referenceChange: 1, maximumError: .2, settlingBand: .1, settlingTimeSeconds: 7200, durationSeconds: 14400 });
    const { request } = engineerVerificationPlan(loop, model, task, { "Main.Kp": 1, "Main.Ki": .001, "Main.Kd": 0 });
    assert.strictEqual(request.max_total_cycles, 144001, "every execution of the 4 h run is simulated");
    // About 200 samples over the 2 h settling time: one sample per 36 s, 360 executions each, about 400 samples in all.
    assert.strictEqual(request.record_every, 360, "the record period follows the process, not the 100 ms task");
    assert.ok(isolatedRecordEvery(request) <= 360, "the process's record period also keeps the run within the sample limits");
    assert.doesNotThrow(() => assertRuntimeRecordingBounds(request));
    // Apply observes the same 4 h response at a record period, so its requirements are valid for that duration too.
    const observed = commissioningObservation(loop, model, task, { "Main.Kp": 1, "Main.Ki": .001, "Main.Kd": 0 });
    assert.strictEqual(validCommissioningObservation(observed, 14400, 100), true, "the 4 h observation's requirements decode");
  });

  test("refuses a declared check beyond the validation run with the duration it needs", () => {
    const loop = pidLoop();
    loop.verificationScenarios = [{ id: "load-trial", initialValues: {}, events: [],
      requirements: [{ id: "late-error", kind: "error", channel: "Main.PV", reference: { channel: "Main.SP" }, statistic: "maximum",
        maximum: .2, window: { start: 20, end: 60 } }] }];
    const model: GeneralModel = { kind: "transfer-function", numerator: [1.5], denominator: [4, 1], delaySeconds: .5,
      inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] },
      source: "provided", validation: "validated", evidence: ["declared-fixture"], revision: "model-4" };
    const task = engineerRequirements({ referenceChange: 1, maximumError: .2, settlingBand: .1, settlingTimeSeconds: 15, durationSeconds: 30 });
    assert.throws(() => engineerVerificationCampaign(loop, model, task, { "Main.Kp": 1, "Main.Ki": .2, "Main.Kd": 0 }), (error: Error) => {
      assert.match(error.message, /load-trial checks late-error until 60 s, after the 30 s validation\. Allow a validation duration of at least 60 s\./);
      assert.strictEqual(classifyMainWorkflowFailure(error).nextAction, "open-requirements", "the fix is in Requirements, not in Tune");
      return true;
    });
  });
});
