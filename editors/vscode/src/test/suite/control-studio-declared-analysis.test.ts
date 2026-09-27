import * as assert from "assert";
import { calculateDeclaredRootLocus, decodeDeclaredAnalysis, type DeclaredControlAnalysis } from "../../controlStudio/declaredAnalysis";

export function declaredShaftAnalysis(): DeclaredControlAnalysis {
  return {
    plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1, 0], delaySeconds: 0,
      inputUnit: "N·m", outputUnit: "rad", revision: "shaft-1", source: "provided", validation: "unvalidated" },
    controller: { numerator: [{ parameter: "Kd" }, { parameter: "Kp" }], denominator: [1],
      parameters: { Kp: { value: 12, minimum: 0, maximum: 100, unit: "N·m/rad" },
        Kd: { value: 7, minimum: 0, maximum: 20, unit: "N·m·s/rad" } } },
    rootLocus: { parameter: "Kp", values: [12, 16, 20], selectedValue: 16 },
  };
}

suite("Declared Control Studio analysis", () => {
  test("allows an exactly fixed parameter while sweeping another gain", () => {
    const original = declaredShaftAnalysis();
    const input = { ...original, controller: { ...original.controller, parameters: { ...original.controller.parameters,
      Kd: { value: 7, minimum: 7, maximum: 7, unit: "N·m·s/rad" } } } };
    assert.ok(decodeDeclaredAnalysis(input), "fixed Kd=7 must not require a writable range");
    assert.strictEqual(calculateDeclaredRootLocus(input).fixedParameters.Kd.value, 7);
  });

  test("computes the declared shaft poles and preserves unvalidated provenance", () => {
    const input = declaredShaftAnalysis();
    const before = JSON.stringify(input);
    const result = calculateDeclaredRootLocus(input);
    assert.strictEqual(result.fixedParameters.Kd.value, 7);
    assert.match(result.provenance, /provided.*unvalidated/);
    assert.deepStrictEqual(result.branches.map(branch => branch.gain), [12, 16, 20]);
    const expected = [[[-6, 0], [-2, 0]], [[-4, 0], [-4, 0]], [[-4, -2], [-4, 2]]];
    result.branches.forEach((branch, index) => branch.poles.forEach((pole, p) => {
      assert.ok(Math.abs(pole.real - expected[index][p][0]) < 1e-6);
      assert.ok(Math.abs(pole.imaginary - expected[index][p][1]) < 1e-6);
    }));
    assert.strictEqual(JSON.stringify(input), before, "calculation must not change the configured controller");
  });

  test("rejects undeclared coefficient operations and singular sweep denominators", () => {
    const input = declaredShaftAnalysis();
    assert.strictEqual(decodeDeclaredAnalysis({ ...input,
      controller: { ...input.controller, numerator: [{ parameter: "Kp", offset: 10 }] },
    }), undefined, "an unsupported coefficient operation must not be silently ignored");
    assert.strictEqual(decodeDeclaredAnalysis({ ...input,
      controller: { ...input.controller, denominator: [{ parameter: "Kp" }] },
      rootLocus: { parameter: "Kp", values: [0, 12], selectedValue: 12 },
    }), undefined, "every sweep denominator must be defined");
  });

  test("rejects unknown parameters, out-of-range candidates and unbounded sweep sizes", () => {
    const input = declaredShaftAnalysis();
    for (const rootLocus of [
      { ...input.rootLocus, parameter: "missing" },
      { ...input.rootLocus, values: [-1, 16] },
      { ...input.rootLocus, values: [] },
      { ...input.rootLocus, values: [12, Number.NaN, 16] },
      { ...input.rootLocus, values: Array(1002).fill(16) },
      { ...input.rootLocus, selectedValue: 18 },
    ]) assert.strictEqual(decodeDeclaredAnalysis({ ...input, rootLocus }), undefined);
    assert.strictEqual(decodeDeclaredAnalysis({ ...input, plant: { ...input.plant, denominator: [0] } }), undefined);
  });
});
