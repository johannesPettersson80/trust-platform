import * as assert from "assert";

suite("Control Studio anti-windup presentation", () => {
  test("shows review, stage, and rollback only in their truthful workflow order", () => {
    const api = presentationApi();
    assert.strictEqual(typeof api.reduceAntiWindupPresentation, "function",
      "the Test result needs a visible state owner before anti-windup controls can be shown");
    let state = api.initialAntiWindupPresentation();
    state = api.reduceAntiWindupPresentation(state, "terminal", { antiWindup: { available: true } });
    assert.deepStrictEqual(state, {
      phase: "ready", visible: true, reviewVisible: true, stageVisible: false, rollbackVisible: false,
      status: "A qualified saturation-return record is ready for one simulation-only tracking candidate.",
    });
    state = api.reduceAntiWindupPresentation(state, "anti-windup", { status: "reviewed", candidate: {
      trackingTimeSeconds: 4, backCalculationGain: 0.25,
    } });
    assert.strictEqual(state.phase, "reviewed");
    assert.strictEqual(state.stageVisible, true);
    assert.match(state.status, /Tt = 4 s.*gain = 0.25.*matched actual-ST/i);
    state = api.reduceAntiWindupPresentation(state, "anti-windup", { status: "compared", comparison: {
      status: "qualified", recommendation: "recommend_tracking", baselineRecoverySeconds: 2, candidateRecoverySeconds: 0.5,
      baselineIntegralChange: 30, candidateIntegralChange: 4, recoveryImprovementSeconds: 1.5, integralReduction: 26,
    } });
    assert.strictEqual(state.phase, "ready");
    assert.match(state.status, /Matched actual-ST comparison.*recommend tracking.*2.*0.5/i);
    state = api.reduceAntiWindupPresentation(state, "anti-windup", { status: "staged" });
    assert.deepStrictEqual({ phase: state.phase, stageVisible: state.stageVisible, rollbackVisible: state.rollbackVisible }, {
      phase: "staged", stageVisible: false, rollbackVisible: true,
    });
    state = api.reduceAntiWindupPresentation(state, "anti-windup", { status: "failed", reason: "runtime changed" });
    assert.strictEqual(state.phase, "staged", "a failed follow-up must not falsely claim the baseline was restored");
    assert.strictEqual(state.status, "runtime changed");
    state = api.reduceAntiWindupPresentation(state, "anti-windup", { status: "rolled-back" });
    assert.strictEqual(state.phase, "ready");
    assert.strictEqual(state.reviewVisible, true);
  });

  test("keeps controls hidden when Test evidence is missing, incomplete, or unsupported", () => {
    const api = presentationApi();
    for (const antiWindup of [undefined, { available: false, reason: "typed adapter missing" }]) {
      const state = api.reduceAntiWindupPresentation(api.initialAntiWindupPresentation(), "terminal", { antiWindup });
      assert.strictEqual(state.visible, false);
    }
  });
});

function presentationApi(): any {
  try { return require("../../controlStudio/antiWindupPresentation"); } catch { return {}; }
}
