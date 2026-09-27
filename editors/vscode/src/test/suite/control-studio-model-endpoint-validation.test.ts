import * as assert from "assert";
import { identifyLowOrderModels } from "../../controlStudio/numerics";

suite("Control Studio independent response endpoint validation", () => {
  const options = { allowedKinds: ["integrating", "first_order"] as ("integrating" | "first_order")[],
    absoluteTolerance: 1e-9, minimumSamples: 20, minimumInputSpan: 1e-9, maximumNormalizedValidationError: 0.15 };
  const samples = (amplitude: number, integrating: boolean) => Array.from({ length: 101 }, (_, index) => {
    const timeSeconds = index * 0.02, elapsed = Math.max(0, timeSeconds - 0.2);
    return { timeSeconds, input: index < 10 ? 0 : amplitude, quality: "good" as const,
      output: amplitude * (integrating ? elapsed : 1 - Math.exp(-elapsed)) };
  });
  test("rejects an integrating approximation whose mean error passes but final response misses the limit", () => {
    const fitted = identifyLowOrderModels(samples(1, false), samples(0.7, false), options);
    assert.strictEqual(fitted.status, "qualified");
    if (fitted.status !== "qualified") return;
    assert.ok(fitted.models.find(model => model.kind === "integrating")!.normalizedValidationError < 0.15,
      "The regression must expose the full-response RMS blind spot");
    assert.strictEqual(fitted.recommended.kind, "first_order",
      "Independent final-response error must disqualify the integrating approximation");
  });
  test("retains the simplest integrating model when its full and final response meet the limit", () => {
    const fitted = identifyLowOrderModels(samples(1, true), samples(0.7, true), options);
    assert.strictEqual(fitted.status, "qualified");
    if (fitted.status === "qualified") assert.strictEqual(fitted.recommended.kind, "integrating");
  });
});
