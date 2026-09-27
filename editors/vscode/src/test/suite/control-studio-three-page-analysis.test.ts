import * as assert from "assert";
import type { DeclaredControlAnalysis } from "../../controlStudio/declaredAnalysis";

const declaredFixture: DeclaredControlAnalysis = {
  plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1, 0], delaySeconds: 0,
    inputUnit: "N·m", outputUnit: "rad", revision: "shaft-j1-b1", source: "provided", validation: "unvalidated" },
  controller: { numerator: [{ parameter: "Kp" }], denominator: [1], parameters: { Kp: { value: 2, minimum: 0, maximum: 8, unit: "N·m/rad" } } },
  rootLocus: { parameter: "Kp", values: [0, 2, 4, 8], selectedValue: 2 },
};

/** This test intentionally loads the new presentation owner dynamically so the
 * pre-fix run reaches the behavior assertion even before the module exists. */
suite("Control Studio three-page analysis presentation", () => {
  test("provides one externally selected contextual analysis disclosure", () => {
    let render: (() => string) | undefined;
    try {
      render = (require("../../controlStudio/threePageAnalysisTemplate") as { renderThreePageAnalysisTemplate?: () => string }).renderThreePageAnalysisTemplate;
    } catch {
      // The expected pre-fix failure is the assertion below, not a module-load failure.
    }
    const markup = render?.() ?? "";
    assert.match(markup, /data-three-page-analysis/);
    assert.doesNotMatch(markup, /data-three-page-analysis-tool/);
    assert.doesNotMatch(markup, /provided model|read-only/i);
    assert.match(markup, /<details[^>]*data-three-page-analysis-table-details[^>]*hidden><summary>Calculation details<\/summary>/);
    assert.deepStrictEqual(declaredFixture.rootLocus.values, [0, 2, 4, 8], "the selected declared sweep remains a typed fixture");
  });

  test("keeps automatic as the default and exposes no runtime-write action", () => {
    let markup = "";
    try {
      const { renderThreePageAnalysisTemplate } = require("../../controlStudio/threePageAnalysisTemplate") as {
        renderThreePageAnalysisTemplate?: () => string;
      };
      markup = renderThreePageAnalysisTemplate?.() ?? "";
    } catch {
      // The expected pre-fix failure is the assertion below, not a module-load failure.
    }
    assert.match(markup, /hidden/);
    assert.doesNotMatch(markup, /data-.*(?:apply|write|runtime)/i);
    assert.doesNotMatch(markup, /Calculate|Import model|Parameter value/i);
  });

  test("exposes bounded periodic measurement and lead-lag owner seams", () => {
    let operations: { estimatePeriodicFrequency?: Function; calculateLeadLag?: Function } | undefined;
    try { operations = require("../../controlStudio/threePageAnalysisOperations") as typeof operations; } catch { operations = undefined; }
    assert.strictEqual(typeof operations?.estimatePeriodicFrequency, "function");
    assert.strictEqual(typeof operations?.calculateLeadLag, "function");
    const input = Array.from({ length: 64 }, (_, index) => Math.sin(index * 0.1 * 5));
    const measured = operations!.estimatePeriodicFrequency!(input, input.map(value => 2 * value), 0.1, [5], { segmentCount: 2, minimumCoherence: 0 });
    assert.strictEqual(measured.status, "qualified");
    const lead = operations!.calculateLeadLag!({ numerator: [1], denominator: [1, 1], delaySeconds: 0 }, "lead", 1, 30);
    assert.strictEqual(lead.status, "qualified");
  });

});
