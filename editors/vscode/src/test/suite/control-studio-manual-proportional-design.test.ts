import * as assert from "assert";

type ManualDesign = (model: unknown, proportionalGain: number, requirements: unknown) => {
  status: "exploratory";
  candidate: { applied: false; structure: "P"; proportionalGain: number };
  selectedModel: { revision: string; provenance: string };
  formulas: { symbolic: readonly string[]; substituted: readonly string[] };
  prediction: {
    evidence: "model-predicted";
    closedLoopPole: number;
    steadyErrorBound: number | null;
    settlingResidualBound: number | null;
    noAsymptoticBoundReason?: string;
  };
  actualSTVerify: true;
  scope: string;
};

const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -100, maximum: 100 } });
const requirements = (bounds = { minimum: -2, maximum: 2 }) => ({
  referenceChange: 1,
  constantInputDisturbance: 0,
  maximumSteadyError: 0.1,
  settlingBand: 0.02,
  settlingTimeSeconds: 2,
  proportionalGainBounds: bounds,
});
const model = (revision: string, source: "provided" | "fitted" = "provided") => ({
  revision,
  source,
  validation: source === "fitted" ? "validated" : "unvalidated",
  evidence: source === "fitted" ? ["independent-validation"] : [],
  delaySeconds: 0,
  kind: "transfer-function",
  inputs: [channel("pump-command", "V")],
  outputs: [channel("tank-level", "m")],
  numerator: [1],
  denominator: [1, 1],
});

suite("Control Studio manual proportional design", () => {
  test("preserves pinned integrating gains and distinguishes stable, boundary and unstable predictions", () => {
    const plant = { ...model("integrator"), denominator: [1, 0], numerator: [2],
      operatingPoint: { inputs: [0.4], outputs: [10] } };
    let stable: ReturnType<ManualDesign> | undefined;
    assert.doesNotThrow(() => { stable = manualDesign()(plant, 2, requirements()); });
    assert.strictEqual(stable!.candidate.proportionalGain, 2);
    assert.strictEqual(stable!.prediction.closedLoopPole, -4);
    assert.strictEqual(stable!.prediction.steadyErrorBound, 0.2);
    assert.ok(Math.abs(stable!.prediction.settlingResidualBound! - 1.2 * Math.exp(-8)) < 1e-12);
    assert.strictEqual(stable!.actualSTVerify, true);
    for (const gain of [0, -1]) {
      const result = manualDesign()(plant, gain, requirements());
      assert.strictEqual(result.status, "exploratory");
      assert.strictEqual(result.candidate.proportionalGain, gain);
      assert.strictEqual(result.prediction.closedLoopPole, gain === 0 ? 0 : 2);
      assert.strictEqual(result.prediction.steadyErrorBound, null);
      assert.strictEqual(result.prediction.settlingResidualBound, null);
    }
    assert.throws(() => manualDesign()(plant, 3, requirements()), /bounds/i);
  });

  test("keeps an in-range entered gain even when its model prediction misses declared task targets", () => {
    const design = manualDesign();
    const result = design(model("sha256:field-pump"), 0.5, requirements());

    assert.strictEqual(result.status, "exploratory");
    assert.deepStrictEqual(result.candidate, { applied: false, structure: "P", proportionalGain: 0.5 });
    assert.deepStrictEqual(result.selectedModel, { revision: "sha256:field-pump", provenance: "declared-provided-unvalidated" });
    assert.strictEqual(result.prediction.evidence, "model-predicted");
    assert.strictEqual(result.prediction.closedLoopPole, -1.5);
    assert.strictEqual(result.prediction.steadyErrorBound, 2 / 3);
    assert.strictEqual(result.prediction.settlingResidualBound, Math.exp(-3));
    assert.strictEqual(result.prediction.noAsymptoticBoundReason, undefined);
    assert.strictEqual(result.actualSTVerify, true);
    assert.ok(result.formulas.symbolic.some(line => line.includes("q = K Kp")));
    assert.ok(result.formulas.substituted.some(line => line.includes("Kp = 0.5")));
    assert.match(result.scope, /Verify/i);
  });

  test("keeps an unstable entered gain exploratory without inventing asymptotic bounds", () => {
    const design = manualDesign();
    const result = design(model("sha256:negative-pole", "fitted"), -1, requirements());

    assert.deepStrictEqual(result.candidate, { applied: false, structure: "P", proportionalGain: -1 });
    assert.deepStrictEqual(result.selectedModel, { revision: "sha256:negative-pole", provenance: "declared-fitted-validated" });
    assert.strictEqual(result.prediction.closedLoopPole, 0);
    assert.strictEqual(result.prediction.steadyErrorBound, null);
    assert.strictEqual(result.prediction.settlingResidualBound, null);
    assert.match(result.prediction.noAsymptoticBoundReason ?? "", /no asymptotic/i);
    assert.strictEqual(result.actualSTVerify, true);
  });

  test("uses the amplified stable negative-q reference transient bound", () => {
    const design = manualDesign();
    const result = design(model("sha256:negative-q"), -0.75, requirements());

    assert.strictEqual(result.prediction.closedLoopPole, -0.25);
    assert.strictEqual(result.prediction.steadyErrorBound, 4);
    assert.strictEqual(result.prediction.settlingResidualBound, 3 * Math.exp(-0.5));
    assert.ok(result.formulas.symbolic.some(line => line.includes("max(1, |q/(1 + q)|)")));
  });

  test("rejects a nonfinite or out-of-range manually entered gain without clamping it", () => {
    const design = manualDesign();
    assert.throws(() => design(model("sha256:bounds"), 2.1, requirements()), /bounds/i);
    assert.throws(() => design(model("sha256:finite"), Number.NaN, requirements()), /finite/i);
  });

  test("rejects numerical overflow instead of classifying it as an unstable pole", () => {
    const design = manualDesign();
    const overflowing = { ...model("sha256:overflow"), numerator: [Number.MAX_VALUE] };
    assert.throws(() => design(overflowing, 2, requirements()), /numerical uncertainty/i);
  });

  test("rejects an underflowed pole when the finite closed-loop factor is nonzero", () => {
    const design = manualDesign();
    const underflowing = { ...model("sha256:underflow"), denominator: [Number.MAX_VALUE, 1] };
    assert.throws(() => design(underflowing, -0.9999999999999998, requirements()), /numerical uncertainty/i);
  });
});

function manualDesign(): ManualDesign {
  let design: ManualDesign | undefined;
  try { design = require("../../controlStudio/manualProportionalDesign").designManualProportionalCandidate as ManualDesign; } catch { /* expected before implementation */ }
  assert.strictEqual(typeof design, "function", "manual P must produce an unapplied exploratory candidate from the selected model");
  return design!;
}
