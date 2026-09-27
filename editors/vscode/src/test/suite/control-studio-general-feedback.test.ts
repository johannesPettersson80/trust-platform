import * as assert from "assert";
import { analyzeGeneralFeedback } from "../../controlStudio/generalFeedback";
const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -100, maximum: 100 } });
const base = { revision: "model-1", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0 };
const plant = { ...base, kind: "continuous-state-space", inputs: [channel("torque", "Nm")], outputs: [channel("angle", "rad")], states: [channel("angle", "rad"), channel("speed", "rad/s")], a: [[0, 1], [0, -8]], b: [[0], [1]], c: [[1, 0]], d: [[0]] };
const controller = { ...base, kind: "transfer-function", inputs: [channel("error", "rad")], outputs: [channel("command", "Nm")], numerator: [12], denominator: [1] };
const definition = () => ({ plant, controller, plantChannel: { inputId: "torque", outputId: "angle" }, controllerChannel: { inputId: "error", outputId: "command" }, feedback: "negative" });
suite("Control Studio explicit feedback analysis", () => {
  test("closes a custom shaft channel with exact fixed-dynamics poles", () => {
    const result = analyzeGeneralFeedback(definition(), [1, 2, 4]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.closedLoop.status, "qualified");
    assert.strictEqual(result.closedLoop.stable, true);
    assert.deepStrictEqual(result.closedLoop.poles.map((p: any) => Math.round(p.real)).sort((a: number, b: number) => a - b), [-6, -2]);
    assert.ok(Math.abs(result.openLoop[0].real + 12 / 65) < 1e-9);
    assert.ok(Math.abs(result.openLoop[0].imaginary + 96 / 65) < 1e-9);
    assert.match(result.nyquistScope, /finite|sampled/i);
  });
  test("retains pure delay without pretending finite poles or full MIMO stability", () => {
    const input = definition();
    input.plant = { ...plant, delaySeconds: 0.5 };
    const result = analyzeGeneralFeedback(input, [1, 2, 4]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.closedLoop.status, "unavailable");
    assert.match(result.closedLoop.reason, /delay/);
  });
  test("rejects unit/domain mismatches and singular feedthrough before closure", () => {
    const units = definition();
    units.controller = { ...controller, outputs: [channel("command", "V")] };
    assert.strictEqual(analyzeGeneralFeedback(units, [1, 2]).status, "rejected");
    const algebraic = { ...definition(), plant: { ...controller, inputs: [channel("torque", "Nm")], outputs: [channel("angle", "rad")], numerator: [2] }, controller: { ...controller, numerator: [-0.5] } };
    const singular = analyzeGeneralFeedback(algebraic, [1, 2]);
    assert.strictEqual(singular.status, "rejected");
    assert.strictEqual(singular.reason, "singular-algebraic-loop");
  });
  test("does not mistake delayed feedthrough for an instantaneous algebraic loop", () => {
    const input = { ...definition(), plant: { ...controller, inputs: [channel("torque", "Nm")], outputs: [channel("angle", "rad")], numerator: [2], delaySeconds: 0.5 }, controller: { ...controller, numerator: [-0.5] } };
    const result = analyzeGeneralFeedback(input, [1, 2]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.closedLoop.status, "unavailable");
  });
  test("labels finite-range classical margin estimates without inventing infinite gain margin", () => {
    const input = { ...definition(), plant: { ...controller, inputs: [channel("torque", "Nm")], outputs: [channel("angle", "rad")], numerator: [1], denominator: [1, 1] }, controller: { ...controller, numerator: [2] } };
    const result = analyzeGeneralFeedback(input, [0.1, 1, 2, 10]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.margins.status, "estimated");
    assert.ok(Math.abs(result.margins.gainCrossoverRadPerSecond - Math.sqrt(3)) < 0.01);
    assert.ok(Math.abs(result.margins.phaseMarginDegrees - 120) < 0.1);
    assert.strictEqual(result.margins.gainMarginDb, undefined);
  });
  test("does not estimate margins when pure-delay phase exceeds the bounded grid resolution", () => {
    const input = { ...definition(), plant: { ...controller, inputs: [channel("torque", "Nm")], outputs: [channel("angle", "rad")], numerator: [1], denominator: [1, 1], delaySeconds: 100 }, controller: { ...controller, numerator: [2] } };
    const result = analyzeGeneralFeedback(input, [0.1, 1, 10]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.margins.status, "unavailable");
    assert.ok(result.nyquistStability, "the Nyquist owner must retain the declared delay instead of substituting zero delay");
  });
  test("qualifies discrete closed-loop poles only with matching sample times", () => {
    const p = { ...base, kind: "discrete-state-space", sampleTimeSeconds: 0.1, inputs: [channel("torque", "Nm")], outputs: [channel("angle", "rad")], states: [channel("x", "rad")], a: [[0.8]], b: [[0.2]], c: [[1]], d: [[0]] };
    const c = { ...p, inputs: [channel("error", "rad")], outputs: [channel("command", "Nm")], a: [[0.5]], b: [[0.1]], c: [[1]], d: [[0.2]] };
    const result = analyzeGeneralFeedback({ ...definition(), plant: p, controller: c }, [1, 2]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.closedLoop.status, "qualified");
    assert.strictEqual(result.closedLoop.stable, true);
    assert.ok(Math.abs(result.closedLoop.poles[0].real - 0.63) < 1e-9);
    assert.strictEqual(analyzeGeneralFeedback({ ...definition(), plant: p, controller: { ...c, sampleTimeSeconds: 0.2 } }, [1, 2]).status, "rejected");
  });
});
