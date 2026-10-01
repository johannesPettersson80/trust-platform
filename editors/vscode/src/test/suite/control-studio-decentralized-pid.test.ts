import * as assert from "assert";

/** Loaded dynamically so that a missing design owner fails at the behaviour assertion. */
function design(): any {
  try { return require("../../controlStudio/decentralizedPidDesign"); } catch { return {}; }
}

const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -100, maximum: 100 } });
/** Two zones sharing a wall: 10·dTᵢ/dt = 0.3·uᵢ − Tᵢ − 0.5·(Tᵢ − Tⱼ), so K = [[0.225, 0.075], [0.075, 0.225]] °C/%. */
const zones = { kind: "continuous-state-space", a: [[-0.15, 0.05], [0.05, -0.15]], b: [[0.03, 0], [0, 0.03]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]],
  states: [channel("T1", "degC"), channel("T2", "degC")], inputs: [channel("u1", "%"), channel("u2", "%")], outputs: [channel("y1", "degC"), channel("y2", "degC")],
  delaySeconds: 0, revision: "zones", source: "fitted", validation: "validated", evidence: ["independent record"] };
const pairs = [{ inputId: "u1", outputId: "y1" }, { inputId: "u2", outputId: "y2" }];

suite("Control Studio decentralized PI design for PID groups", () => {
  test("designs each member's PI by SIMC on its own path with the gain it sees when the other loops are closed", () => {
    const api = design();
    assert.strictEqual(typeof api.designDecentralizedPi, "function", "a PID group needs a model-based design, not only a declared grid");
    const result = api.designDecentralizedPi(zones, pairs, "balanced");
    assert.strictEqual(result.members.length, 2);
    for (const member of result.members) {
      assert.ok(Math.abs(member.pathGain - 0.225) < 1e-3, `own-path gain ${member.pathGain}`);
      assert.ok(Math.abs(member.relativeGain - 1.125) < 1e-3, `relative gain ${member.relativeGain}`);
      assert.ok(Math.abs(member.effectiveGain - member.pathGain / member.relativeGain) < 1e-12, "Kᵢᵢ/λᵢᵢ");
      // y(t) = 0.225 − 0.15·e^(−0.1t) − 0.075·e^(−0.2t): t₂₈ ≈ 2.55 s and t₆₃ ≈ 7.97 s give a negative delay, so θ = 0 and τ = t₆₃.
      assert.strictEqual(member.delaySeconds, 0);
      assert.ok(Math.abs(member.timeConstantSeconds - 7.97) < 0.1, `time constant ${member.timeConstantSeconds}`);
      const closedLoop = 2 * Math.max(member.delaySeconds, member.timeConstantSeconds / 3);
      const kp = member.timeConstantSeconds / (member.effectiveGain * (closedLoop + member.delaySeconds));
      const close = (actual: number, expected: number) => Math.abs(actual - expected) <= 1e-7 * Math.abs(expected);
      assert.ok(close(member.proportionalGain, kp), `SIMC Kp ${member.proportionalGain} ≠ ${kp}`);
      assert.ok(close(member.integralGain, kp / Math.min(member.timeConstantSeconds, 4 * (closedLoop + member.delaySeconds))), "SIMC Ti");
      assert.strictEqual(member.derivativeGain, 0);
    }
  });

  test("seeds every member of a PID group from the decentralized design", () => {
    const api = design();
    assert.strictEqual(typeof api.designDecentralizedPi, "function");
    const { projectPidGroupLoop } = require("./control-studio-pid-group-fixtures");
    const loop = projectPidGroupLoop();
    loop.controller.project.seed = { kind: "decentralized-pid", family: "PI" };
    const { decodeProjectController } = require("../../controlStudio/projectController");
    assert.ok(decodeProjectController(loop.controller.project, loop.parameterBinding), "a group declares the decentralized seed instead of a gain grid");
    const model = { ...zones, inputs: [channel("u1", "%"), channel("u2", "%")], outputs: [channel("y1", "°C"), channel("y2", "°C")] };
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const task = { referenceChange: 1, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 30, durationSeconds: 60, tuningObjective: "balanced" };
    const values = projectControllerSeed(loop, model, task, loop.controller.project.seed, "PI");
    const [first, second] = api.designDecentralizedPi(model, pairs, "balanced").members;
    assert.deepStrictEqual(values, {
      "P1.Parameters.ProportionalGain": first.proportionalGain, "P1.Parameters.IntegralGain": first.integralGain, "P1.Parameters.DerivativeGain": 0,
      "P1.SecondaryParameters.ProportionalGain": second.proportionalGain, "P1.SecondaryParameters.IntegralGain": second.integralGain,
      "P1.SecondaryParameters.DerivativeGain": 0,
    }, "each member's own parameter record receives its own design");
  });

  test("refuses a pairing with a negative relative gain and names the pairing to use", () => {
    const api = design();
    assert.strictEqual(typeof api.designDecentralizedPi, "function");
    // K = [[0.2, 0.5], [0.5, 0.2]]: λ₁₁ = 1/(1 − 0.25/0.04) ≈ −0.19; the crossed pairing has λ ≈ 1.19.
    const crossed = { ...zones, a: [[-1, 0], [0, -1]], b: [[0.2, 0.5], [0.5, 0.2]], revision: "crossed" };
    assert.throws(() => api.designDecentralizedPi(crossed, pairs, "balanced"),
      /relative gain of u1 → y1 is -0\.19.*pair u1 → y2 and u2 → y1/i);
  });
});
