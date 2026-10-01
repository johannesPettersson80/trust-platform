import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { noiseAwareTransferTolerance } from "../../controlStudio/proportionalTransfer";
import { recordedHalfRange, restingNoiseProbeRequest, restingOperatingModel } from "../../controlStudio/transferNoiseProbe";

function lessonLoop(): any {
  const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  return JSON.parse(fs.readFileSync(path.join(root,
    "examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque/control-loop.json"), "utf8"));
}

suite("Control Studio Auto-entry noise allowance", () => {
  test("allows the derivative's and integral's response to measurement noise at Auto entry", () => {
    const loop = lessonLoop(), noise = 0.001;
    // One cycle can move the output by Kp·2n (P on Δy), Ki·Δt·n (I) and Kd·4n/Δt (D on the change of the sampled rate).
    const pid = noiseAwareTransferTolerance(loop, undefined, 16, noise, { integralGain: 12, derivativeGain: 7, cycleSeconds: 0.02 });
    assert.ok(Math.abs(pid - (2 * 16 * noise + 12 * 0.02 * noise + 4 * 7 * noise / 0.02)) < 1e-12, `PID allowance ${pid}`);
    assert.ok(Math.abs(noiseAwareTransferTolerance(loop, undefined, 16, noise) - 2 * 16 * noise) < 1e-12, "P alone keeps its allowance");
    assert.strictEqual(noiseAwareTransferTolerance(loop, 0.5, 16, noise, { derivativeGain: 7, cycleSeconds: 0.02 }), 0.5, "a reviewed tolerance wins");
    assert.ok(noiseAwareTransferTolerance(loop, undefined, 16, undefined, { derivativeGain: 7, cycleSeconds: 0.02 }) < 1e-4,
      "without measured noise the check stays exact");
  });

  test("probes the resting noise of the transfer trial's plant", () => {
    const request = { execution: "isolated-simulation", step_ms: 20, duration_ms: 120, max_total_cycles: 7,
      initial_values: { "P1.AutoMode": 0 }, events: [{ at_ms: 60, symbol: "P1.AutoMode", value: 1 }], observations: [] };
    const probe = restingNoiseProbeRequest(request);
    assert.deepStrictEqual(probe.events, [], "the probe stays in Manual: no transfer");
    assert.strictEqual(probe.duration_ms, 1000, "one second at rest");
    assert.strictEqual(probe.max_total_cycles, 51);
    assert.deepStrictEqual(probe.initial_values, request.initial_values, "from the same held operating point");
    const run = { samples: [0.0004, -0.0009, 0.0007, -0.0002].map((value, index) => ({ timeSeconds: index * 0.02,
      values: { "P1.AngleRad": { value, quality: "good" } } })) };
    assert.ok(Math.abs(recordedHalfRange({ run } as any, "P1.AngleRad")! - 0.0008) < 1e-12, "half of the recorded range");
  });

  test("starts the transfer check where the configured plant actually rests", () => {
    const model: any = { kind: "transfer-function", numerator: [1.44], denominator: [3.7, 1], delaySeconds: 0.7, revision: "fitted",
      source: "fitted", validation: "validated", evidence: ["a", "b"], operatingPoint: { inputs: [0], outputs: [20.128] },
      inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }], outputs: [{ id: "y", unit: "°C", operatingRange: { minimum: 0, maximum: 100 } }] };
    const resting: any = { run: { samples: [0, 0.1, 0.2, 0.3, 0.4].map((timeSeconds, sequence) => ({ timeSeconds, sequence,
      values: { "P1.ProcessValue": { value: 20, quality: "good" }, "P1.ManipulatedValue": { value: 0, quality: "good" } } })) } };
    const plant = restingOperatingModel(model, resting, ["P1.ProcessValue"]);
    assert.deepStrictEqual(plant.operatingPoint, { inputs: [0], outputs: [20] }, "the fitted 20.128 °C is replaced by the plant's rest");
    assert.deepStrictEqual((plant as any).numerator, model.numerator, "only the starting point changes; the dynamics stay the fitted ones");
    assert.deepStrictEqual(model.operatingPoint.outputs, [20.128], "the selected model itself is unchanged");
    assert.strictEqual(restingOperatingModel(model, { run: { samples: [] } } as any, ["P1.ProcessValue"]), model, "without a resting measurement the model is kept");
  });
});
