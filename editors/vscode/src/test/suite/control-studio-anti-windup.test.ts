import * as assert from "assert";

suite("Control Studio anti-windup", () => {
  test("calculates the retained back-calculation term from the actual limited output", () => {
    const api = antiWindupApi();
    assert.strictEqual(typeof api.calculateBackCalculationStep, "function",
      "anti-windup needs an explicit trial/request/clamp/correction calculation");
    assert.deepStrictEqual(api.calculateBackCalculationStep({
      retainedIntegral: 80,
      integralGain: 0.5,
      error: 10,
      cycleSeconds: 0.1,
      proportionalContribution: 20,
      outputMinimum: 0,
      outputMaximum: 100,
      trackingTimeSeconds: 1,
    }), {
      trialIntegral: 80.5,
      requestedOutput: 100.5,
      appliedOutput: 100,
      trackingCorrection: -0.05,
      nextIntegral: 80.45,
    });
    assert.deepStrictEqual(api.calculateBackCalculationStep({
      retainedIntegral: 75,
      integralGain: 0.5,
      error: 10,
      cycleSeconds: 0.1,
      proportionalContribution: 20,
      outputMinimum: 0,
      outputMaximum: 100,
      trackingTimeSeconds: 1,
    }), {
      trialIntegral: 75.5,
      requestedOutput: 95.5,
      appliedOutput: 95.5,
      trackingCorrection: 0,
      nextIntegral: 75.5,
    }, "tracking must not correct when request already equals applied output");
  });

  test("rejects a nonpositive tracking time or an invalid output range", () => {
    const api = antiWindupApi();
    for (const changed of [
      { trackingTimeSeconds: 0 }, { trackingTimeSeconds: Number.NaN },
      { outputMinimum: 100, outputMaximum: 100 },
    ]) assert.throws(() => api.calculateBackCalculationStep({
      retainedIntegral: 80, integralGain: 0.5, error: 10, cycleSeconds: 0.1,
      proportionalContribution: 20, outputMinimum: 0, outputMaximum: 100,
      trackingTimeSeconds: 1, ...changed,
    }), /tracking time|output limits/i);
  });
});

function antiWindupApi(): any {
  try { return require("../../controlStudio/antiWindup"); } catch { return {}; }
}
