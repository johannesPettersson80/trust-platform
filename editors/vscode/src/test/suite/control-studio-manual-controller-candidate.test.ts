import * as assert from "assert";
import { tuneControllerAdapter } from "../../controlStudio/controllerTuning";
import { pidLoop, channel } from "./control-studio-pid-fixtures";
import type { GeneralModel } from "../../controlStudio/generalModel";
import { decodeEngineerStudy } from "../../controlStudio/engineerStudyPersistence";
import { restoredEngineerView } from "../../controlStudio/restoredEngineerView";

suite("Control Studio manual controller candidate", () => {
  test("retains the selected tuning method and manual values on reopening without restoring application eligibility", () => {
    const tuningPreferences = { method: "manual", values: { "Main.Kp": 1.23456789012345, "Main.Ki": .7890123456789 } };
    const saved = decodeEngineerStudy({ campaigns: [], tuningPreferences });
    assert.ok(saved, "The saved study retains the engineer's tuning method and values");
    const reopened = restoredEngineerView(saved!) as any;
    assert.deepStrictEqual(reopened.tuningPreferences, tuningPreferences);
    assert.strictEqual(reopened.stageEvidence.validationPassed, false);
    assert.strictEqual(decodeEngineerStudy({ campaigns: [], tuningPreferences: { method: "manual", values: { "Main.Kp": Infinity } } }), undefined);
  });
  const model: GeneralModel = { kind: "transfer-function", numerator: [2], denominator: [4, 1], delaySeconds: 0,
    revision: "measured-model", source: "fitted", validation: "validated", evidence: ["independent-data"],
    inputs: [channel("Main.MV", "V")], outputs: [channel("Main.PV", "m")] };
  const task = { referenceChange: 1, maximumError: .1, settlingBand: .02, settlingTimeSeconds: 12, durationSeconds: 24 };
  test("uses the engineer's complete PI values without running an automatic tuning rule", () => {
    const result = tuneControllerAdapter(pidLoop(), model, task, { family: "PI", applicationMode: "manual",
      manualValues: { "Main.Kp": 1.23456789012345, "Main.Ki": .7890123456789 } } as any);
    assert.strictEqual(result.family, "PI");
    assert.deepStrictEqual(result.values, { "Main.Kp": 1.23456789012345, "Main.Ki": .7890123456789, "Main.Kd": 0 });
    assert.strictEqual(result.design.candidate.applied, false);
    assert.strictEqual(result.design.selectedModel.revision, model.revision);
  });
  test("rejects missing, incompatible and out-of-bounds manual values", () => {
    for (const manualValues of [{ "Main.Kp": 1 }, { "Main.Kp": 1, "Main.Ki": 101 }, { "Main.Kp": 1, "Main.Ki": .5, "Main.Kd": 2 }]) {
      assert.throws(() => tuneControllerAdapter(pidLoop(), model, task, { family: "PI", applicationMode: "manual", manualValues } as any), /setting|gain|bound|parameter|family|integral|derivative/i);
    }
  });
});
