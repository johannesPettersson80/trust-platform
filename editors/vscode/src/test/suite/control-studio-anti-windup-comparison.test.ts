import * as assert from "assert";

suite("Control Studio matched anti-windup comparison", () => {
  test("compares the tracked record only when both saturation-return conditions are identical", () => {
    const api = comparisonApi();
    assert.strictEqual(typeof api.compareAntiWindupRuns, "function",
      "WF-008 needs an immutable matched-run comparison before it can recommend tracking");
    const result = api.compareAntiWindupRuns({
      baseline: run({ trackingGain: 0, recoverySeconds: 2, integralChange: 30, oscillationDetected: false }),
      candidate: run({ trackingGain: 0.25, recoverySeconds: 0.5, integralChange: 4, oscillationDetected: false }),
    });
    assert.deepStrictEqual(result, {
      status: "qualified", recommendation: "recommend_tracking",
      baselineRecoverySeconds: 2, candidateRecoverySeconds: 0.5,
      baselineIntegralChange: 30, candidateIntegralChange: 4,
      recoveryImprovementSeconds: 1.5, integralReduction: 26,
      reason: "The matched tracked record reduces retained-integral excess and leaves saturation sooner without recorded oscillation.",
    });
  });

  test("rejects mismatched reset evidence, a tracked baseline, or candidate oscillation", () => {
    const api = comparisonApi();
    const baseline = run({ trackingGain: 0, recoverySeconds: 2, integralChange: 30, oscillationDetected: false });
    const candidate = run({ trackingGain: 0.25, recoverySeconds: 0.5, integralChange: 4, oscillationDetected: false });
    assert.deepStrictEqual(api.compareAntiWindupRuns({ baseline, candidate: { ...candidate, conditions: { ...candidate.conditions,
      controllerInitialConditionDigest: "different-reset" } } }), {
      status: "inconclusive", reason: "The candidate did not start from the same retained controller state.",
    });
    assert.deepStrictEqual(api.compareAntiWindupRuns({ baseline: { ...baseline, trackingGain: 0.1 }, candidate }), {
      status: "inconclusive", reason: "The baseline must prove an untracked back-calculation gain of zero.",
    });
    assert.deepStrictEqual(api.compareAntiWindupRuns({ baseline, candidate: run({ trackingGain: 0.25, recoverySeconds: 0.5,
      integralChange: 4, oscillationDetected: true }) }), {
      status: "qualified", recommendation: "do_not_recommend_tracking",
      baselineRecoverySeconds: 2, candidateRecoverySeconds: 0.5,
      baselineIntegralChange: 30, candidateIntegralChange: 4,
      recoveryImprovementSeconds: 1.5, integralReduction: 26,
      reason: "The matched tracked record has recorded oscillation; do not recommend it despite the recovery metrics.",
    });
  });
});

function comparisonApi(): any {
  try { return require("../../controlStudio/antiWindupComparison"); } catch { return {}; }
}

function run(input: { trackingGain: number; recoverySeconds: number; integralChange: number; oscillationDetected: boolean }): any {
  return {
    trackingGain: input.trackingGain,
    conditions: {
      loopId: "loop:heater", loopRevision: "loop-v1", source: "simulation", scenarioDigest: "scenario-35-45-35",
      plantInitialConditionDigest: "plant-hot-reset", controllerInitialConditionDigest: "controller-untracked-reset",
      samplePeriodMs: 100, durationMs: 70000, outputMinimum: 0, outputMaximum: 100,
    },
    assessment: { status: "qualified", limit: "upper", enteredAtSeconds: 1, targetReturnAtSeconds: 10,
      releasedAtSeconds: 10 + input.recoverySeconds, recoverySeconds: input.recoverySeconds,
      peakRequestedOutput: 125, peakAppliedOutput: 100, maximumMismatch: 25,
      integralAtEntry: 75, integralAtTargetReturn: 75 + input.integralChange,
      integralAtRelease: 75, integralChangeDuringSaturation: input.integralChange, recommendation: "Candidate needed." },
    oscillationDetected: input.oscillationDetected,
  };
}
