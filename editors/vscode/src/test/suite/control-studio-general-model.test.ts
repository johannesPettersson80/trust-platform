import * as assert from "assert";
import { analyzeGeneralModelFrequency, decodeGeneralModel } from "../../controlStudio/generalModel";

const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -10, maximum: 10 } });
const base = () => ({ revision: "engineer-model-1", source: "provided", validation: "unvalidated",
  evidence: [], delaySeconds: 0, inputs: [channel("heater", "W"), channel("fan", "V")],
  outputs: [channel("temperature", "degC")], states: [channel("thermal", "degC")],
  kind: "continuous-state-space", a: [[-2]], b: [[1, 3]], c: [[2]], d: [[0, 1]] });

suite("Control Studio general models", () => {
  test("connects supported SISO continuous state-space models to declared analysis", () => {
    const api = require("../../controlStudio/selectedModelAnalysis") as { selectedModelAnalysis: (declared: unknown, selected: unknown) => any };
    const selected = { ...base(), inputs: [channel("u", "V")], outputs: [channel("y", "m")],
      states: [channel("x1", "m"), channel("x2", "m/s")], a: [[0, 1], [-2, -3]], b: [[0], [1]], c: [[1, 0]], d: [[0]] };
    const declared = { plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      inputUnit: "V", outputUnit: "m", revision: "declared", source: "provided", validation: "validated" },
      controller: { numerator: [1], denominator: [1], parameters: { Kp: { value: 1, minimum: 0, maximum: 2, unit: "V/m" } } },
      rootLocus: { parameter: "Kp", values: [1], selectedValue: 1 } };
    const connected = api.selectedModelAnalysis(declared, selected);
    assert.deepStrictEqual(connected.plant.denominator, [1, 3, 2]);
    assert.strictEqual(connected.plant.revision, selected.revision, "analysis uses the selected model identity while preserving its full state denominator");
  });
  test("converts SISO continuous state-space without cancelling hidden modes", () => {
    let api: any = {};
    try { api = require("../../controlStudio/linearTransferModel"); } catch { /* expected missing behavior */ }
    assert.strictEqual(typeof api.linearTransferModel, "function");
    const model = { ...base(), inputs: [channel("u", "V")], outputs: [channel("y", "m")],
      states: [channel("x1", "m"), channel("x2", "m/s")],
      a: [[0, 1], [-2, -3]], b: [[0], [1]], c: [[1, 0]], d: [[0]] };
    const converted = api.linearTransferModel(model);
    assert.deepStrictEqual(converted.denominator, [1, 3, 2]);
    assert.deepStrictEqual(converted.numerator, [1]);
    assert.strictEqual(converted.revision, model.revision);
    const hidden = api.linearTransferModel({ ...model, a: [[-1, 0], [0, 1]], b: [[1], [0]], c: [[1, 0]] });
    assert.deepStrictEqual(hidden.denominator, [1, 0, -1]);
    assert.deepStrictEqual(hidden.numerator, [1, -1]);
    assert.throws(() => api.linearTransferModel(base()), /SISO|single input/i);
    assert.throws(() => api.linearTransferModel({ ...model, kind: "discrete-state-space", sampleTimeSeconds: 0.1 }), /sampled.*controller/i);
  });

  test("analyzes an unvalidated rectangular supplied model without a runtime controller", () => {
    const model = base();
    assert.ok(decodeGeneralModel(model), "a supplied rectangular model must be accepted");
    const result = analyzeGeneralModelFrequency(model, [2]);
    assert.strictEqual(result.status, "qualified");
    assert.deepStrictEqual(result.channels.map((c: any) => [c.output.id, c.input.id]),
      [["temperature", "heater"], ["temperature", "fan"]]);
    assert.ok(Math.abs(result.channels[0].points[0].real - 0.5) < 1e-9);
    assert.ok(Math.abs(result.channels[0].points[0].imaginary + 0.5) < 1e-9);
    assert.ok(Math.abs(result.channels[1].points[0].real - 2.5) < 1e-9);
    assert.strictEqual(result.capabilities.relativeGain.available, false);
    assert.strictEqual(result.model.validation, "unvalidated");
    assert.deepStrictEqual(result.poles, [{ real: -2, imaginary: 0 }]);
  });
  test("calculates controllability and observability for the declared two-tank model", () => {
    const model = { ...base(), inputs: [channel("flow", "m3/s")], outputs: [channel("level", "m")],
      states: [channel("upstream", "m"), channel("downstream", "m")],
      a: [[-0.04,0],[0.04,-0.04]], b: [[2],[0]], c: [[0,1]], d: [[0]] };
    const result = analyzeGeneralModelFrequency(model, [0.1,1]) as any;
    assert.strictEqual(result.stateStructure?.controllability.rank, 2);
    assert.deepStrictEqual(result.stateStructure.controllability.matrix, [[2,-0.08],[0,0.08]]);
    assert.deepStrictEqual(result.stateStructure.observability.matrix, [[0,1],[0.04,-0.04]]);
    assert.strictEqual(result.stateStructure.observability.rank, 2);
    const hidden = analyzeGeneralModelFrequency({ ...model, c: [[1,0]] }, [0.1,1]) as any;
    assert.strictEqual(hidden.stateStructure.controllability.rank, 2);
    assert.strictEqual(hidden.stateStructure.observability.rank, 1);
  });
  test("distinguishes discrete dynamics and rejects aliasing", () => {
    const model = { ...base(), kind: "discrete-state-space", sampleTimeSeconds: 0.1,
      a: [[0.5]], b: [[1, 0]], c: [[1]], d: [[0, 0]] };
    const result = analyzeGeneralModelFrequency(model, [Math.PI / 2 / 0.1]);
    assert.strictEqual(result.status, "qualified");
    assert.ok(Math.abs(result.channels[0].points[0].real + 0.4) < 1e-9);
    assert.ok(Math.abs(result.channels[0].points[0].imaginary + 0.8) < 1e-9);
    assert.strictEqual(result.stable, true);
    const alias = analyzeGeneralModelFrequency(model, [Math.PI / 0.1]);
    assert.strictEqual(alias.status, "rejected");
    assert.strictEqual(alias.reason, "alias-risk");
  });
  test("analyzes supplied transfer functions with exact frequency delay", () => {
    const model = { ...base(), kind: "transfer-function", inputs: [base().inputs[0]],
      numerator: [2], denominator: [1, 2], delaySeconds: Math.PI / 4 };
    const result = analyzeGeneralModelFrequency(model, [2]);
    assert.strictEqual(result.status, "qualified");
    assert.ok(Math.abs(result.channels[0].points[0].real + 0.5) < 1e-9);
    assert.ok(Math.abs(result.channels[0].points[0].imaginary + 0.5) < 1e-9);
    assert.ok(result.poleScope.includes("delay"));
  });
  test("qualifies square DC relative gain but rejects a singular frequency", () => {
    const model = { ...base(), inputs: [base().inputs[0]] , b: [[1]], d: [[0]] };
    const result = analyzeGeneralModelFrequency(model, [2]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.capabilities.relativeGain.available, true);
    assert.deepStrictEqual(result.relativeGainMatrix, [[1]]);
    const singular = { ...base(), states: [base().states[0], channel("second", "degC")],
      a: [[0, -1], [1, 0]], b: [[1, 0], [0, 1]], c: [[1, 0]], d: [[0, 0]] };
    assert.strictEqual(analyzeGeneralModelFrequency(singular, [1]).status, "rejected");
  });
  test("keeps supplied evidence and channel units in a separate validated snapshot", () => {
    const source = { ...base(), validation: "validated", evidence: ["run-29-validation"] };
    const decoded = decodeGeneralModel(source);
    assert.ok(decoded);
    source.inputs[0].unit = "changed";
    source.evidence[0] = "changed";
    assert.strictEqual(decoded.inputs[0].unit, "W");
    assert.deepStrictEqual(decoded.evidence, ["run-29-validation"]);
    const result = analyzeGeneralModelFrequency({ ...base(), inputs: [base().inputs[0]], b: [[0]], d: [[0]] }, [1]);
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.capabilities.relativeGain.available, false);
    assert.deepStrictEqual(result.channels[0].points[0], {
      frequencyRadPerSecond: 1, real: 0, imaginary: 0, magnitude: 0, phaseRadians: 0 });
  });
  test("rejects malformed, unsupported and falsely validated model contracts", () => {
    for (const model of [{ ...base(), b: [[1]] }, { ...base(), a: [[NaN]] },
      { ...base(), validation: "validated" }, { ...base(), kind: "discrete-state-space", sampleTimeSeconds: 0 },
      { ...base(), inputs: [base().inputs[0], base().inputs[0]] },
      { ...base(), kind: "transfer-function", numerator: [1], denominator: [0] }]) {
      assert.strictEqual(decodeGeneralModel(model), undefined);
    }
    const invalid = analyzeGeneralModelFrequency(base(), [0]);
    assert.strictEqual(invalid.status, "rejected");
    assert.strictEqual(invalid.reason, "invalid-frequency-grid");
    assert.strictEqual(analyzeGeneralModelFrequency({ ...base(), a: [[0]] }, [1]).status, "qualified");
  });
});
