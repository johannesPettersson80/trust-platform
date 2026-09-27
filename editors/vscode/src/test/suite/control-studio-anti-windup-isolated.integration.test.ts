import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { RuntimeVerificationClient } from "../../controlStudio/verificationClient";

suite("Control Studio isolated anti-windup actual-ST workflow", () => {
  test("runs matched untracked and tracked saturation-return records against the same ST source", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_ANTI_WINDUP !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(project, "the anti-windup runner must open its non-course fixture");
    const loop = JSON.parse(fs.readFileSync(path.join(project!, "control-loop.json"), "utf8"));
    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    try {
      const search = await require("../../controlStudio/runtimeSearchConnection").connectRuntimeSearch(loop);
      assert.strictEqual(search.source, "simulation");
      const plan = require("../../controlStudio/antiWindupVerificationPlan").buildAntiWindupVerificationPlan({
        loop, baselineValues: { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 },
        trackingTimeSeconds: 4,
        saturationWindup: qualifiedFinding(),
      });
      const verifier = new RuntimeVerificationClient({ source: "simulation", request: search.request });
      const baseline = await verifier.verify(plan.baseline, plan.requirements);
      const candidate = await verifier.verify(plan.candidate, plan.requirements);
      assert.strictEqual(baseline.run.evidence, "ST-executed");
      assert.strictEqual(candidate.run.evidence, "ST-executed");
      assert.deepStrictEqual(baseline.run.conditions, candidate.run.conditions,
        "the actual ST records must use identical initial values, events, timing and plant source");
      assert.deepStrictEqual(baseline.run.parameters, { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0 });
      assert.deepStrictEqual(candidate.run.parameters, { "Main.Kp": 2, "Main.Ki": 0.5, "Main.Kd": 0, "Main.BackCalculationGain": 0.25 });
      const baselineRequested = channelMaximum(baseline.run, "Main.RequestedOutput");
      const baselineApplied = channelMaximum(baseline.run, "Main.MV");
      assert.ok(baselineRequested > 100, `the unlimited request must remain observable: ${baselineRequested}`);
      assert.strictEqual(baselineApplied, 100, `the applied output must remain clamped: ${baselineApplied}`);
      assert.ok(channelMaximum(candidate.run, "Main.Integral") < channelMaximum(baseline.run, "Main.Integral"),
        "the tracked ST record must retain less integral excess under the same saturation scenario");
      const toEvidence = require("../../controlStudio/antiWindupRecordedRun").antiWindupRunFromRecordedSt;
      const comparison = require("../../controlStudio/antiWindupComparison").compareAntiWindupRuns({
        baseline: toEvidence(loop, baseline.run), candidate: toEvidence(loop, candidate.run),
      });
      assert.strictEqual(comparison.status, "qualified", JSON.stringify(comparison));
      assert.strictEqual(comparison.baselineRecoverySeconds >= 0, true);
      assert.strictEqual(comparison.candidateRecoverySeconds >= 0, true);
    } finally {
      await runtimeLifecycleService.stopRuntime();
    }
  });
});

function channelMaximum(run: any, symbol: string): number {
  return Math.max(...run.samples.map((sample: any) => sample.values[symbol]?.value).filter(Number.isFinite));
}

function qualifiedFinding() {
  return { status: "qualified", limit: "upper", enteredAtSeconds: 0.1, targetReturnAtSeconds: 2,
    releasedAtSeconds: 3, recoverySeconds: 1, peakRequestedOutput: 125, peakAppliedOutput: 100, maximumMismatch: 25,
    integralAtEntry: 75, integralAtTargetReturn: 90, integralAtRelease: 85, integralChangeDuringSaturation: 15,
    recommendation: "Compare a tracked candidate." };
}
