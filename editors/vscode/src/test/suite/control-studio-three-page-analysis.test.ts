import * as assert from "assert";
import { readEditorInputs } from "../../controlStudio/webview/analysisEntries";
import { formatMatrixRows, formatNumberList } from "../../controlStudio/webview/numberListText";

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

  test("sends every entry of an analysis as the text the engineer typed, so that the engine refuses what it cannot read", () => {
    // The engine reads each entry (analysis/entries.rs): a blank one is absent, and an unreadable one is refused under its field. The page
    // drops none of them, so the grid is sent whole with its unreadable entry in place, and the list with its unreadable item.
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>
      <input data-analysis-minimum-frequency value="0.1"><input data-analysis-maximum-frequency value="abc"><input data-analysis-frequency-points>
      <input data-analysis-periodic-frequencies value="1, x">
      <select data-analysis-lead-lag-mode><option value="lead">Lead</option><option value="lag" selected>Lag</option></select>
      <input data-analysis-lead-lag-crossover value="1"><input data-analysis-lead-lag-ratio value="">
    </main>`);
    try {
      const editor = dom.window.document.querySelector("main");
      assert.deepStrictEqual(readEditorInputs(editor, "frequency"), { grid: { minimumFrequency: "0.1", maximumFrequency: "abc", points: "" } });
      assert.deepStrictEqual(readEditorInputs(editor, "measuredFrequency"), { frequencies: "1, x", segmentCount: "", minimumCoherence: "", startupSamples: "" });
      assert.deepStrictEqual(readEditorInputs(editor, "leadLag"), { mode: "lag", crossoverRadPerSecond: "1", value: "" });
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
});
