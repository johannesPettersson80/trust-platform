import * as assert from "assert";
import { routhDetailText } from "../../controlStudio/webview/routhDetails";

type DeclaredControlAnalysis = {
  plant: { kind: "transfer-function"; numerator: readonly number[]; denominator: readonly number[]; delaySeconds: number;
    inputUnit: string; outputUnit: string; revision: string; source: "provided" | "simulation"; validation: "unvalidated" | "validated" };
  controller: { numerator: readonly (number | { parameter: string; scale?: number })[];
    denominator: readonly (number | { parameter: string; scale?: number })[];
    parameters: Readonly<Record<string, { value: number; minimum: number; maximum: number; unit: string }>> };
  rootLocus: { parameter: string; values: readonly number[]; selectedValue: number };
};

type RouthModule = {
  analyzeRouthPolynomial: (coefficients: readonly number[]) => any;
  calculateDeclaredRouth: (input: DeclaredControlAnalysis, selectedValue?: number) => any;
};

function loadRouth(): RouthModule | undefined {
  try {
    return require("../../controlStudio/routhHurwitz") as RouthModule;
  } catch (error) {
    if ((error as { code?: string }).code === "MODULE_NOT_FOUND") return undefined;
    throw error;
  }
}


function declaredAffineSecondOrder(): DeclaredControlAnalysis {
  return {
    plant: { kind: "transfer-function", numerator: [1], denominator: [1, 3, 2], delaySeconds: 0,
      inputUnit: "V", outputUnit: "m", revision: "generic-2", source: "provided", validation: "validated" },
    controller: { numerator: [{ parameter: "gain" }], denominator: [1],
      parameters: { gain: { value: 1, minimum: -10, maximum: 10, unit: "1" } } },
    rootLocus: { parameter: "gain", values: [-1, 1, 4], selectedValue: 1 },
  };
}

suite("Control Studio Routh-Hurwitz mathematics", () => {
  test("constructs generic tables and counts established right-half-plane poles", () => {
    const routh = loadRouth();
    assert.strictEqual(typeof routh?.analyzeRouthPolynomial, "function",
      "Routh analysis must be exported from the isolated mathematical module");
    const stable = routh!.analyzeRouthPolynomial([1, 5, 6]);
    assert.strictEqual(stable.status, "qualified");
    assert.deepStrictEqual(stable.rows.map((row: { power: number; entries: readonly number[] }) => [row.power, row.entries]),
      [[2, [1, 6]], [1, [5, 0]], [0, [6, 0]]]);
    assert.strictEqual(stable.signChanges, 0);
    assert.strictEqual(stable.stable, true);
    assert.match(stable.interpretation, /asymptotically stable/);
    const unstable = routh!.analyzeRouthPolynomial([1, 2, 1, 4]);
    assert.deepStrictEqual(unstable.firstColumn.map((entry: { sign: string }) => entry.sign),
      ["positive", "positive", "negative", "positive"]);
    assert.strictEqual(unstable.signChanges, 2);
    assert.strictEqual(unstable.stable, false);
  });

  test("names Routh rows with superscript powers of s", () => {
    const routh = loadRouth();
    const zeroPivot = routh!.analyzeRouthPolynomial([1, 0, 1, 2, 3]);
    const explanations = zeroPivot.issues.map((issue: { explanation: string }) => issue.explanation);
    assert.ok(explanations.some((text: string) => /\bs[²³⁴]/.test(text)), `a row is named by its power: ${explanations.join(" | ")}`);
    assert.ok(explanations.every((text: string) => !text.includes("^")), `no caret notation: ${explanations.join(" | ")}`);
  });

  test("rejects invalid polynomials and keeps Routh boundary cases explicit", () => {
    const routh = loadRouth();
    assert.strictEqual(typeof routh?.analyzeRouthPolynomial, "function");
    assert.strictEqual(routh!.analyzeRouthPolynomial([0, 1]).status, "rejected");
    assert.strictEqual(routh!.analyzeRouthPolynomial([0, 0]).status, "rejected");
    assert.strictEqual(routh!.analyzeRouthPolynomial([1, Number.NaN]).status, "rejected");
    const origin = routh!.analyzeRouthPolynomial([1, 1, 0]);
    assert.strictEqual(origin.signChanges, 0, "Exact auxiliary handling counts RHP poles while retaining the excluded origin boundary");
    assert.strictEqual(origin.stable, false);
    assert.ok(origin.issues.some((issue: { kind: string }) => issue.kind === "origin-pole"));
    assert.ok(origin.issues.some((issue: { kind: string }) => issue.kind === "all-zero-row"));
    assert.match(origin.interpretation, /origin pole/);
    assert.match(origin.interpretation, /auxiliary polynomial/, "the all-zero row is resolved by the auxiliary polynomial, not by ε");
    assert.doesNotMatch(origin.interpretation, /formal/);
    const zeroRow = origin.issues.find((issue: { kind: string }) => issue.kind === "all-zero-row");
    assert.match(zeroRow.explanation, /derivative/, "the resolved all-zero row names its replacement");
    assert.doesNotMatch(zeroRow.explanation, /requires separate analysis/);
    const details = routhDetailText(origin);
    assert.match(details ?? "", /^First-column signs of the exact table: positive, positive, positive\./);
    assert.match(details ?? "", /zero constant coefficient establishes an origin pole/);
    assert.match(details ?? "", /derivative/);
    const zeroPivot = routh!.analyzeRouthPolynomial([1, 0, 1, 2, 3]);
    assert.strictEqual(zeroPivot.status, "qualified", "an exact zero pivot should use the symbolic ε→0+ rule");
    assert.strictEqual(zeroPivot.signChanges, 2);
    assert.ok(zeroPivot.issues.some((issue: { kind: string }) => issue.kind === "zero-pivot"));
    const laterZeroPivot = routh!.analyzeRouthPolynomial([1, -3, -1, 3, -3]);
    assert.strictEqual(laterZeroPivot.status, "qualified", "a later zero pivot should use the formal ε recurrence");
    assert.strictEqual(laterZeroPivot.signChanges, 3);
    assert.ok(laterZeroPivot.issues.some((issue: { kind: string }) => issue.kind === "zero-pivot"));
    const symmetricRealPoles = routh!.analyzeRouthPolynomial([1, 0, -1]);
    assert.strictEqual(symmetricRealPoles.status, "qualified", "an auxiliary row with real symmetric roots is not an imaginary-axis boundary");
    assert.strictEqual(symmetricRealPoles.stable, false);
    const terminalNearZero = routh!.analyzeRouthPolynomial([1, 1, 1e-13]);
    assert.strictEqual(terminalNearZero.status, "uncertain");
    assert.ok(terminalNearZero.issues.some((issue: { kind: string }) => issue.kind === "near-zero-entry"));
  });

  test("derives affine first and second order intervals without sampled sweeps", () => {
    const routh = loadRouth();
    assert.strictEqual(typeof routh?.calculateDeclaredRouth, "function");
    const input = declaredAffineSecondOrder();
    const before = JSON.stringify(input);
    const secondOrder = routh!.calculateDeclaredRouth(input, -1);
    assert.deepStrictEqual(secondOrder.polynomial.coefficients, [1, 3, 1]);
    assert.strictEqual(secondOrder.interval.status, "qualified");
    assert.strictEqual(secondOrder.interval.kind, "open");
    assert.strictEqual(secondOrder.interval.lower, -2);
    assert.strictEqual(secondOrder.interval.upper, undefined);
    assert.deepStrictEqual(secondOrder.allowedDomain, { minimum: -10, maximum: 10 });
    assert.match(secondOrder.provenance, /provided.*generic-2.*validated/);
    assert.strictEqual(JSON.stringify(input), before, "analysis must not mutate the declared model");
    const firstOrder = { ...declaredAffineSecondOrder(), plant: { ...declaredAffineSecondOrder().plant, denominator: [1, 2] } };
    assert.strictEqual(routh!.calculateDeclaredRouth(firstOrder).interval.lower, -2);
    const tinySlope = declaredAffineSecondOrder();
    tinySlope.plant = { ...tinySlope.plant, denominator: [1, 1] };
    tinySlope.controller = { ...tinySlope.controller, numerator: [{ parameter: "gain", scale: 1e-15 }] };
    assert.strictEqual(routh!.calculateDeclaredRouth(tinySlope).interval.status, "uncertain");
    const impossible = declaredAffineSecondOrder();
    impossible.plant = { ...impossible.plant, denominator: [1, 0] };
    impossible.controller = { ...impossible.controller, numerator: [0] };
    assert.strictEqual(routh!.calculateDeclaredRouth(impossible).interval.kind, "empty");
  });

  test("rejects delay and solves parameter-dependent leading coefficient regions", () => {
    const routh = loadRouth();
    assert.strictEqual(typeof routh?.calculateDeclaredRouth, "function");
    const delayed = declaredAffineSecondOrder();
    delayed.plant.delaySeconds = 0.1;
    assert.strictEqual(routh!.calculateDeclaredRouth(delayed).polynomial.status, "rejected");
    const variableLeading: DeclaredControlAnalysis = {
      ...declaredAffineSecondOrder(),
      plant: { ...declaredAffineSecondOrder().plant, denominator: [1, 0] },
      controller: { numerator: [1], denominator: [{ parameter: "gain" }],
        parameters: { gain: { value: 1, minimum: 1, maximum: 10, unit: "1" } } },
      rootLocus: { parameter: "gain", values: [1, 2], selectedValue: 1 },
    };
    const interval = routh!.calculateDeclaredRouth(variableLeading).interval;
    assert.strictEqual(interval.status, "qualified");
    assert.strictEqual(interval.lower, 0);
    assert.strictEqual(interval.upper, undefined);
  });

  test("solves a complete cubic one-parameter stability interval", () => {
    const routh = loadRouth();
    assert.strictEqual(typeof routh?.calculateDeclaredRouth, "function");
    const input: DeclaredControlAnalysis = {
      ...declaredAffineSecondOrder(),
      plant: { ...declaredAffineSecondOrder().plant, denominator: [1, 2, 3, 0] },
      controller: { numerator: [{ parameter: "gain" }], denominator: [1],
        parameters: { gain: { value: 1, minimum: -10, maximum: 10, unit: "1" } } },
    };
    const result = routh!.calculateDeclaredRouth(input, 1);
    assert.strictEqual(result.interval.status, "qualified");
    assert.strictEqual(result.interval.kind, "open");
    assert.strictEqual(result.interval.lower, 0);
    assert.strictEqual(result.interval.upper, 6);
    assert.strictEqual(routh!.calculateDeclaredRouth(input, 0).polynomial.stable, false,
      "the zero-gain boundary must not be reported as asymptotically stable");
    assert.strictEqual(routh!.calculateDeclaredRouth(input, 6).polynomial.stable, false,
      "the cubic boundary at K=6 must not be reported as asymptotically stable");
  });

  test("retains disjoint algebraic stability regions and handles degree seven", () => {
    const routh = loadRouth()!;
    const input: DeclaredControlAnalysis = { ...declaredAffineSecondOrder(),
      plant: { ...declaredAffineSecondOrder().plant, denominator: [1, 3, 3, 10] },
      controller: { numerator: [{ parameter: "gain" }, { parameter: "gain" }, { parameter: "gain", scale: 4 }], denominator: [1],
        parameters: { gain: { value: 1, minimum: -10, maximum: 10, unit: "1" } } } };
    const interval = routh.calculateDeclaredRouth(input).interval as any;
    assert.strictEqual(interval.status, "qualified");
    assert.strictEqual(interval.kind, "union", "Both stable components must remain available");
    assert.strictEqual(interval.intervals.length, 2);
    assert.ok(Math.abs(interval.intervals[0].lower + 2.5) < 1e-12);
    assert.ok(Math.abs(interval.intervals[0].upper - (-1 - Math.SQRT2)) < 1e-12);
    assert.ok(Math.abs(interval.intervals[1].lower - (-1 + Math.SQRT2)) < 1e-12);
    assert.ok(interval.boundaries.every((boundary: any) => boundary.polynomial.length && boundary.lower && boundary.upper), "Every algebraic boundary retains an exact isolating certificate");
    input.plant.denominator = [1, 7, 21, 35, 35, 21, 7, 1];
    input.controller.numerator = [{ parameter: "gain" }];
    const seventh = routh.calculateDeclaredRouth(input).interval;
    assert.strictEqual(seventh.status, "qualified", "The old degree-two/six caps cannot bound the course capability");
    assert.ok(seventh.lower! < 0 && seventh.upper! > 0);
  });

  test("handles exact zero pivots and auxiliary all-zero rows without epsilon substitution", () => {
    const routh = loadRouth();
    assert.strictEqual(typeof routh?.analyzeRouthPolynomial, "function");
    const zeroPivot = routh!.analyzeRouthPolynomial([1, 0, 1, 1]);
    assert.strictEqual(zeroPivot.status, "qualified");
    assert.strictEqual(zeroPivot.signChanges, 2);
    assert.strictEqual(zeroPivot.stable, false);
    assert.ok(zeroPivot.issues.some((issue: { kind: string }) => issue.kind === "zero-pivot"));
    const auxiliary = routh!.analyzeRouthPolynomial([1, 0, 2, 0, 1]);
    assert.strictEqual(auxiliary.status, "boundary");
    assert.strictEqual(auxiliary.stable, false);
    assert.ok(auxiliary.issues.some((issue: { kind: string }) => issue.kind === "all-zero-row"));
    assert.ok(auxiliary.interpretation.includes("auxiliary"));
  });

  test("counts known factored poles across repeated, symmetric and scaled singular cases", () => {
    const analyze = loadRouth()!.analyzeRouthPolynomial;
    const product = (a: number[], b: number[]) => { const result = Array(a.length + b.length - 1).fill(0); a.forEach((x, i) => b.forEach((y, j) => { result[i + j] += x * y; })); return result; };
    for (const scale of [1, 1e-9, 1e9]) for (const realRoots of [[-1, -1], [-1, 1], [-2, 0, 2], [-3, -2, -1, 1, 2, 3]]) {
      for (const conjugateReal of [-1, 0, 1]) {
        const coefficients = realRoots.reduce((p, r) => product(p, [1, -r]), [1]);
        const combined = product(coefficients, [1, -2 * conjugateReal, conjugateReal ** 2 + 1]).map(v => scale === 1 ? v : Number(`${v}e${scale === 1e-9 ? -9 : 9}`));
        const result = analyze(combined);
        const boundary = realRoots.includes(0) || conjugateReal === 0;
        assert.strictEqual(result.status, boundary ? "boundary" : "qualified", JSON.stringify(combined));
        assert.strictEqual(result.signChanges, realRoots.filter(r => r > 0).length + (conjugateReal > 0 ? 2 : 0), JSON.stringify(combined));
      }
    }
  });

  test("shows each stability boundary as a decimal and keeps its exact enclosure for inspection", () => {
    const details = require("../../controlStudio/webview/routhDetails");
    assert.strictEqual(typeof details.routhBoundaryRows, "function", "raw rational enclosures are not readable as table values");
    const rows = details.routhBoundaryRows([
      { polynomial: ["2", "15", "35", "19", "-15"], lower: "-6755399441055745/2251799813685248", upper: "-1688849860263935/562949953421312" },
      { polynomial: ["2", "15", "35", "19", "-15"], lower: "-5/2", upper: "-5/2" },
      { polynomial: ["1", "-2"], lower: "3/2", upper: "5/2" },
    ]);
    assert.deepStrictEqual(rows.map((row: any) => [row[0], row[1].text]), [
      ["2, 15, 35, 19, -15", "-3"], ["2, 15, 35, 19, -15", "-2.5"], ["1, -2", "1.5 – 2.5"]]);
    assert.strictEqual(rows[0][1].title, "Exact enclosure [-6755399441055745/2251799813685248, -1688849860263935/562949953421312]");
  });
});
