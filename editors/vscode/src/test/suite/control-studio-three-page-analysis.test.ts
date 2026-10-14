import * as assert from "assert";
import { completedExperimentRecording } from "../../controlStudio/webview/periodicRecording";
import type { DeclaredControlAnalysis } from "../../controlStudio/declaredAnalysis";
import { formatMatrixRows, formatNumberList } from "../../controlStudio/webview/numberListText";
import { renderIdentifiedModel } from "../../controlStudio/webview/threePageModel";

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

  test("writes the engine's state-structure matrices in one precision", () => {
    // The state structure, its ranks, its order and its scope are the engine's answer. The Rust test
    // `the_state_structure_has_the_declared_order_the_ranks_out_of_the_state_count_and_the_matrices`
    // (crates/trust-control/src/analysis/model_views_tests.rs) holds the numbers this test used to check. The page writes each
    // matrix's rows in one precision, and nothing else.
    assert.strictEqual(formatMatrixRows([[-0.04, 0], [0.04, -0.04]]), "[-0.04, 0]; [0.04, -0.04]");
    assert.strictEqual(formatMatrixRows([[2], [0]]), "[2]; [0]");
    assert.strictEqual(formatMatrixRows([[0, 1]]), "[0, 1]");
  });

  test("lists Routh entries and cancelled factors with one precision", () => {
    assert.strictEqual(formatNumberList([12.345, 0.25, 7]), "12.345, 0.25, 7");
    assert.strictEqual(formatNumberList([1, 0.08, 0.0016]), "1, 0.08, 0.0016");
    assert.strictEqual(formatNumberList([]), "");
  });

  test("keeps the analysis presentation free of the numerics the engine answers", () => {
    // The phase unwrapping, the dB and degree conversions, the root computations, the sentences of each answer and the recording
    // rules are the engine's (specification 35, B9.9 and §0.1a; the Rust tests of `analysis/` and `study/`). The analysis
    // presentation draws the engine's answer and computes none of these, so its source may not reach for them.
    const fs = require("fs"), path = require("path");
    const source: string = fs.readFileSync(path.resolve(__dirname, "../../../src/controlStudio/webview/threePageAnalysis.ts"), "utf8");
    for (const moved of [/log10/, /rootsOfRealPolynomial/, /measuredFrequencyRows/, /stateStructureDetails/, /routhDetailText/,
      /decodePeriodicRecording/, /numerics\//, /generalModel/, /routhHurwitz/]) {
      assert.doesNotMatch(source, moved, `the presentation reaches for ${moved}`);
    }
  });

  test("counts a model's channels in the singular for one input or output", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<main><p data-studio-model-type></p><pre data-studio-model-equation></pre><section data-studio-model-result hidden></section></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 1 } });
      renderIdentifiedModel(root, { kind: "continuous-state-space", revision: "r", source: "provided", validation: "validated", evidence: ["e"],
        delaySeconds: 0, inputs: [channel("pump", "m3/s")], outputs: [channel("h1", "m"), channel("h2", "m")], states: [channel("x1", "m"), channel("x2", "m")],
        a: [[-0.04, 0], [0.04, -0.04]], b: [[2], [0]], c: [[1, 0], [0, 1]], d: [[0], [0]] } as any);
      assert.strictEqual(root.querySelector("[data-studio-model-type]").textContent, "Continuous state-space model · 1 input, 2 outputs");
    } finally { dom.window.close(); }
  });

  test("writes powers of s as superscripts in the identified transfer function", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<main><p data-studio-model-type></p><pre data-studio-model-equation></pre><section data-studio-model-result hidden></section></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 1 } });
      renderIdentifiedModel(root, { kind: "transfer-function", revision: "r", source: "provided", validation: "validated", evidence: ["e"],
        numerator: [2], denominator: [6, 5, 1], delaySeconds: 0, inputs: [channel("u", "V")], outputs: [channel("y", "m")] } as any);
      // The equation is a stacked fraction: its numerator and denominator are read separately.
      assert.strictEqual(root.querySelector(".studio-math-num").textContent, "2");
      assert.strictEqual(root.querySelector(".studio-math-den").textContent, "6s² +5s +1");
    } finally { dom.window.close(); }
  });

  test("aligns analysis table headers with their cells and separates the columns", () => {
    const { JSDOM } = require("jsdom");
    const fs = require("fs"), path = require("path");
    const theme = fs.readFileSync(path.resolve(__dirname, "../../../src/webview/theme.css"), "utf8");
    const dom = new JSDOM(`<html><head><style>${theme}</style></head><body><table class="trust-table"><thead><tr><th>Stage</th><th>Gain K</th></tr></thead>
      <tbody><tr><td>0</td><td>[123456.7, 234567.8]</td></tr></tbody></table></body></html>`);
    try {
      const header = dom.window.document.querySelector("th"), cell = dom.window.document.querySelector("td");
      const style = (element: Element) => dom.window.getComputedStyle(element);
      assert.strictEqual(style(header).textAlign, "left", "a header sits over its left-aligned column");
      assert.strictEqual(style(cell).textAlign, "left");
      assert.ok(parseFloat(style(cell).paddingRight) >= 12, `columns are separated, not touching: ${style(cell).paddingRight}`);
    } finally { dom.window.close(); }
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

  test("offers the recording of a completed project experiment, and nothing else, to the analyses", () => {
    // A live recording: the first scan has no previous scan, so its cycle time is unknown; every later scan took 0.1 s.
    const rawSamples = Array.from({ length: 64 }, (_, index) => ({ scan: 100 + index, quality: "good", loopRevision: "loop-1",
      actualCycleSeconds: index === 0 ? null : 0.1, appliedMv: 75 + 5 * Math.sin(0.2 * index * 0.1), pv: 35,
      outputLimits: { minimum: 0, maximum: 100 } }));
    const recording = { rawSamples, evidence: { completeness: "complete", missingScanRanges: [], droppedObservedSamples: 0,
      actualCycleTimingAvailable: true, recordingId: "project-run" } };
    const message = { kind: "control-studio.experiment.result", phase: "terminal", payload: { status: "completed", recording } };
    assert.deepStrictEqual(completedExperimentRecording(message), recording, "the completed experiment's recording is offered to the analyses");
    assert.strictEqual(completedExperimentRecording({ ...message, phase: "running" }), undefined);
    assert.strictEqual(completedExperimentRecording({ ...message, payload: { status: "aborted", recording } }), undefined);
    // The cycle timing, the gaps and the eligibility of a recording for a measurement are the engine's: `measuredFrequency` reads the
    // study's recording and refuses one it cannot measure (B3.6, A9; the Rust tests of `analysis/measured_frequency*`).
  });
});
