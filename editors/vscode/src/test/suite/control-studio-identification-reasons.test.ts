import * as assert from "assert";
import * as path from "path";
import { BoundedAnalysisJob } from "../../controlStudio/boundedAnalysisJob";
import { controllerAdapter } from "../../controlStudio/controllerAdapters";
import { withSimplerModelNote } from "../../controlStudio/engineerActions/find";
import type { ModelComparison } from "../../controlStudio/modelComparison";
import { fitRecordedModel } from "../../controlStudio/recordedModelIdentification";
import type { RuntimeVerificationResult } from "../../controlStudio/verificationClient";
import { pidLoop } from "./control-studio-pid-fixtures";

const STEP_FAILURE = "Step fit did not settle.";
const STATE_FAILURE = "No identifiable adequate state model. Order 1: Estimation fit exceeds the error limit.";

/** One stepped recording: its executed experiment decides which simple model is tried first. */
function steppedRecording(): RuntimeVerificationResult {
  return { run: { id: "step" }, request: { experiment: { kind: "mv_step" } } } as unknown as RuntimeVerificationResult;
}

/** Stands in for BoundedAnalysisJob: each worker either rejects with its own message or resolves with its result. */
function workers(answers: Readonly<Record<string, { error: string } | { result: unknown }>>): BoundedAnalysisJob {
  return {
    run: async (modulePath: string) => {
      const worker = path.basename(modulePath), answer = answers[worker];
      if (!answer) throw new Error(`No fake answer for ${worker}.`);
      if ("error" in answer) throw new Error(answer.error);
      return answer.result;
    },
  } as unknown as BoundedAnalysisJob;
}

suite("Control Studio identification reasons", () => {
  test("names the failed step attempt and the failed state model, in that order", async () => {
    const fitting = workers({
      "provisionalStepIdentificationWorker.js": { error: STEP_FAILURE },
      "generalIdentificationWorker.js": { error: STATE_FAILURE },
    });
    await assert.rejects(fitRecordedModel(pidLoop(), [steppedRecording()], fitting, () => false), (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.strictEqual(error.message, `Step-response fit: ${STEP_FAILURE} State model: ${STATE_FAILURE}`);
      return true;
    });
  });

  test("carries the failed step attempt's reason with a state model that succeeds", async () => {
    const stateModel = { kind: "transfer-function" };
    const fitting = workers({
      "provisionalStepIdentificationWorker.js": { error: STEP_FAILURE },
      "generalIdentificationWorker.js": { result: { model: stateModel, comparisons: [], validationPredictions: [], estimationPredictions: [] } },
    });
    const result = await fitRecordedModel(pidLoop(), [steppedRecording()], fitting, () => false);
    assert.ok("model" in result, "the state model is the result");
    assert.strictEqual(result.model, stateModel);
    assert.deepStrictEqual(result.fallbackReasons, [STEP_FAILURE]);
  });

  test("adds no fallback reason when the step attempt itself succeeds", async () => {
    const fitting = workers({
      "provisionalStepIdentificationWorker.js": { result: { model: { kind: "transfer-function" }, runs: [], fit: {} } },
    });
    const result = await fitRecordedModel(pidLoop(), [steppedRecording()], fitting, () => false);
    assert.strictEqual(result.fallbackReasons, undefined);
  });

  test("names an unrecognised controller type rather than tuning it as a proportional controller", () => {
    const loop = pidLoop();
    loop.controller = { ...loop.controller, type: "Vendor.Mystery" };
    delete loop.controller.law;
    delete loop.analysis;
    assert.throws(() => controllerAdapter(loop), (error: unknown) =>
      error instanceof Error && error.message.startsWith("Controller type Vendor.Mystery is not a recognised ST adapter."));
  });

  test("keeps a declared proportional law on the proportional adapter, whatever the block type", () => {
    const loop = pidLoop();
    loop.controller = { ...loop.controller, type: "Vendor.Mystery", law: { kind: "proportional", gainSymbol: "Main.Kp" } };
    assert.deepStrictEqual(controllerAdapter(loop).families, ["P"]);
  });

  test("shows a simpler model that was not used as one line of the comparison", () => {
    const comparison = { modelRevision: "rev", presentation: { warnings: ["Existing warning."] } } as unknown as ModelComparison;
    assert.deepStrictEqual(withSimplerModelNote(comparison, [STEP_FAILURE]).presentation.warnings,
      ["Existing warning.", "A simpler model was not used: Step fit did not settle."]);
    assert.strictEqual(withSimplerModelNote(comparison, []), comparison);
  });
});
