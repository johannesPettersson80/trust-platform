import * as assert from "assert";

type Design = (model: unknown, requirements: unknown) => {
  status: "provisional";
  candidate: { applied: false; structure: "P"; proportionalGain: number };
  selectedModel: { revision: string; provenance: string };
  formulas: { symbolic: readonly string[]; substituted: readonly string[] };
  prediction: {
    evidence: "model-predicted";
    steadyErrorBound: number;
    settlingResidualBound: number;
    closedLoopPole: number;
  };
  actualSTVerify: true;
};

function design(): Design | undefined {
  try {
    return require("../../controlStudio/simpleControllerDesign").designSimpleProportionalController as Design;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "MODULE_NOT_FOUND") return undefined;
    throw error;
  }
}

const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -100, maximum: 100 } });
const requirements = (bounds = { minimum: -20, maximum: 20 }) => ({
  referenceChange: 1,
  constantInputDisturbance: 0,
  maximumSteadyError: 0.1,
  settlingBand: 0.02,
  settlingTimeSeconds: 2,
  proportionalGainBounds: bounds,
});
const model = (source: "provided" | "fitted", b = 1, delaySeconds = 0) => ({
  revision: `sha256:${source}-${b}-${delaySeconds}`,
  source,
  validation: source === "fitted" ? "validated" : "unvalidated",
  evidence: source === "fitted" ? ["independent-validation"] : [],
  delaySeconds,
  kind: "transfer-function",
  inputs: [channel("command", "V")],
  outputs: [channel("temperature", "degC")],
  numerator: [b],
  denominator: [1, 1],
});

suite("Control Studio simple proportional-controller design", () => {
  test("uses equivalent continuous state-space dynamics without changing selected model identity", () => {
    const stateSpace = { ...model("fitted"), kind: "continuous-state-space", numerator: undefined, denominator: undefined,
      states: [channel("storage", "m")], a: [[-1]], b: [[2]], c: [[0.5]], d: [[0]] };
    let result: ReturnType<Design> | undefined;
    assert.doesNotThrow(() => { result = design()!(stateSpace, requirements()); },
      "equivalent supplied model representations must reach the same controller design");
    assert.strictEqual(result!.candidate.proportionalGain, 9);
    assert.strictEqual(result!.selectedModel.revision, stateSpace.revision);
    assert.strictEqual(stateSpace.kind, "continuous-state-space");
    assert.deepStrictEqual(stateSpace.b, [[2]]);
  });

  test("derives the same provisional P prediction from supplied and fitted first-order models", () => {
    const designSimpleProportionalController = design();
    assert.ok(designSimpleProportionalController, "the selected simple model must yield an unapplied P design");
    for (const source of ["provided", "fitted"] as const) {
      const result = designSimpleProportionalController!(model(source), requirements());
      assert.strictEqual(result.status, "provisional");
      assert.deepStrictEqual(result.candidate, { applied: false, structure: "P", proportionalGain: 9 });
      assert.strictEqual(result.selectedModel.revision, `sha256:${source}-1-0`);
      assert.strictEqual(result.selectedModel.provenance, `declared-${source}-${source === "fitted" ? "validated" : "unvalidated"}`);
      assert.strictEqual(result.prediction.evidence, "model-predicted");
      assert.ok(result.prediction.steadyErrorBound <= 0.1);
      assert.ok(result.prediction.settlingResidualBound <= 0.02);
      assert.strictEqual(result.prediction.closedLoopPole, -10);
      assert.strictEqual(result.actualSTVerify, true);
      assert.ok(result.formulas.symbolic.some(line => line.includes("q = K Kp")));
      assert.ok(result.formulas.substituted.some(line => line.includes("q = 9")));
    }
  });

  test("connects supplied and fitted integrating models to bounded P tuning", () => {
    for (const source of ["provided", "fitted"] as const) {
      for (const rate of [2, -2]) {
        const integrating = { ...model(source, rate), denominator: [1, 0],
          operatingPoint: { inputs: [0.4], outputs: [10] } };
        let result: ReturnType<Design> | undefined;
        assert.doesNotThrow(() => { result = design()!(integrating, requirements()); },
          "an integrating model must reach the connected proportional design");
        assert.ok(result!.candidate.proportionalGain * rate > 0);
        assert.ok(result!.prediction.steadyErrorBound <= 0.1 + 1e-12);
        assert.ok(result!.prediction.settlingResidualBound <= 0.02 + 1e-12);
        assert.strictEqual(result!.prediction.closedLoopPole, -rate * result!.candidate.proportionalGain);
        assert.strictEqual(result!.actualSTVerify, true);
      }
    }
    assert.throws(() => design()!({ ...model("provided"), denominator: [1, 0] },
      requirements({ minimum: 0, maximum: 0.01 })), /bounds|satisfy/i);
  });

  test("uses the controller sign required by a negative-gain plant", () => {
    const designSimpleProportionalController = design();
    assert.ok(designSimpleProportionalController, "the selected simple model must yield an unapplied P design");
    const result = designSimpleProportionalController!(model("provided", -2), requirements());
    assert.strictEqual(result.candidate.proportionalGain, -4.5);
    assert.strictEqual(result.prediction.closedLoopPole, -10);
    assert.ok(result.prediction.steadyErrorBound <= 0.1);
  });

  test("rejects unavailable bounded and delayed design paths without weakening the requirement", () => {
    const designSimpleProportionalController = design();
    assert.ok(designSimpleProportionalController, "the selected simple model must yield an unapplied P design");
    assert.throws(
      () => designSimpleProportionalController!(model("provided"), requirements({ minimum: 0, maximum: 8 })),
      /bounds/i,
    );
    assert.throws(
      () => designSimpleProportionalController!(model("provided", 1, 0.1), requirements()),
      /delay/i,
    );
  });
  test("verifies the entered response deadline against the required target-error band", () => {
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const fs = require("fs"), path = require("path");
    const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8"));
    const model = { kind: "transfer-function", numerator: [1], denominator: [1,1], delaySeconds: 0,
      revision: "model", source: "provided", validation: "validated", evidence: ["independent-validation"],
      inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
      outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
    const plan = engineerVerificationPlan(loop, model, { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
      settlingTimeSeconds: 2, durationSeconds: 4 }, { "Main.Kp": 11.5 });
    const settling = plan.requirements.find((requirement: { kind: string }) => requirement.kind === "settling");
    assert.deepStrictEqual(settling, { id: "response-deadline", kind: "settling", channel: "Main.PV",
      target: 1, band: 0.1, deadlineSeconds: 2, holdSeconds: 2, window: { start: 0, end: 4 } },
      "Verify must report actual settling time and enforce the requested deadline within the target-error band");
  });

});
