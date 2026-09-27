import * as assert from "assert";
import { buildEngineeringPlotEvidence } from "../../controlStudio/engineeringPlots";
import { candidateFromControlPid } from "../../controlStudio/controllerCandidate";

suite("Control Studio prediction coordinates", () => {
  test("uses the physical operating point, target change, and controller period", () => {
    const baseline = process.env.TRUST_CONTROL_STUDIO_PREDICTION_BASELINE;
    const predict: typeof buildEngineeringPlotEvidence = baseline ? require(baseline).buildEngineeringPlotEvidence : buildEngineeringPlotEvidence;
    const controller = candidateFromControlPid({ proportionalGain: 2, integralGain: 0, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -20, outputMaximum: 20, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation" }, "Manual");
    const model = { kind: "first_order_dead_time" as const, gain: 1, timeConstantSeconds: 1, deadTimeSeconds: 0, normalizedError: 0, stable: true };
    const result = predict(model, controller, controller, undefined, {
      cycleTimeSeconds: 0.02, durationSeconds: 4, referenceChange: 2, operatingPoint: { input: 2, output: 2 },
    });
    assert.strictEqual(result.timeResponse[0].candidatePv, 2, "the trace begins at the declared physical process value");
    assert.strictEqual(result.timeResponse[0].candidateMv, 2);
    assert.strictEqual(result.timeResponse[1].timeSeconds, 0.02);
    assert.strictEqual(result.timeResponse[result.timeResponse.length - 1].timeSeconds, 4);
    const final = result.timeResponse[result.timeResponse.length - 1].candidatePv;
    assert.ok(Math.abs(final - 8 / 3) < 1e-4, "P=2 acting on absolute target 4 converges to 8/3, not a shifted unit-step approximation");
  });
});
