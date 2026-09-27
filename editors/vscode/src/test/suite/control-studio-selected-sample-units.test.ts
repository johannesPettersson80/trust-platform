/// <reference lib="dom.iterable" />
import * as assert from "assert";
import { ControlChartAdapter } from "../../controlStudio/webview/liveChart";
import { liveSignals } from "../../controlStudio/webview/liveSignalPlot";
import { chartTheme } from "../../controlStudio/webview/chartPrimitives";
import { color } from "echarts/core";
import type { ControlChartSample } from "../../controlStudio/chartContract";

suite("Control Studio selected sample announcement", () => {
  test("renders retained sample values with independent signal quality and scoped headers", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<div><table><thead><tr></tr></thead><tbody></tbody></table></div>');
    const original = globalThis.document;
    globalThis.document = dom.window.document;
    try {
      const samples: ControlChartSample[] = [{ elapsedSeconds: 1, sp: 40, pv: 35, mv: 75, quality: "stale",
        additional: [{ id: "request", displayName: "Requested power", channel: "heater", role: "MV", unit: "%", value: 110, quality: "forced" },
          { id: "state", displayName: "Stored correction", channel: "heater", role: "controller-state", unit: "%", value: null, quality: "bad" }] },
        { elapsedSeconds: 2, sp: 40, pv: 36, mv: 80, quality: "good", additional: [] }];
      const table = dom.window.document.querySelector("div");
      const owner = Object.assign(Object.create(ControlChartAdapter.prototype), {
        table, tableBody: table.querySelector("tbody"), samples,
        signals: liveSignals(samples, "°C", "%"), selectedSignals: new Set(["sp", "pv", "mv", "request", "state"]),
      });
      owner.renderTable();
      const row = table.querySelector("tbody tr");
      assert.match(row.children[1].textContent, /40.*Stale/);
      assert.match(row.children[4].textContent, /110.*Forced/);
      assert.match(row.children[5].textContent, /Missing.*Bad/);
      assert.match(table.querySelectorAll("tbody tr")[1].children[4].textContent, /Missing.*Unavailable/);
      assert.strictEqual(table.querySelectorAll("tbody tr")[1].children[1].querySelector(".sr-only")?.textContent, "Good");
      assert.deepStrictEqual([...table.querySelectorAll("th")].map((cell: HTMLTableCellElement) => [cell.textContent, cell.scope]),
        ["Elapsed time (s)", "SP (°C)", "PV (°C)", "MV (%)", "Requested power (%)", "Stored correction (%)"].map(label => [label, "col"]));
      owner.selectedSignals.delete("request"); owner.renderTable();
      assert.ok(!table.querySelector("thead").textContent.includes("Requested power"), "deselected signals leave the table too");
    } finally { globalThis.document = original; dom.window.close(); }
  });
  test("preserves theme hues while making translucent diagnostic strokes visible", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<style>:root{--vscode-charts-orange:rgba(234,92,0,0.33)}</style>');
    const original = { document: globalThis.document, getComputedStyle: globalThis.getComputedStyle };
    Object.assign(globalThis, { document: dom.window.document, getComputedStyle: dom.window.getComputedStyle.bind(dom.window) });
    try {
      const stroke = color.parse(chartTheme().auxiliaryOrange)!;
      assert.deepStrictEqual(stroke.slice(0, 3), [234, 92, 0], "keep the user's theme hue");
      assert.strictEqual(stroke[3], 1, "a translucent theme fill must not dim an actual measured trace");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });
  test("announces selected SP PV and MV using their actual engineering units", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<p role="status"></p>');
    try {
      const status = dom.window.document.querySelector("p");
      const owner = Object.assign(Object.create(ControlChartAdapter.prototype), {
        status, selectedSample: 0, samples: [{ elapsedSeconds: 1, sp: 1, pv: 0.5, mv: 2 }],
        processUnit: "m", outputUnit: "V",
        signals: liveSignals([], "m", "V"), selectedSignals: new Set(["sp", "pv", "mv"]),
      });
      owner.announceSelectedSample();
      assert.match(status.textContent, /SP 1 m, PV 0.5 m, MV 2 V/);
      assert.ok(!status.textContent.includes("percent"));
    } finally { dom.window.close(); }
  });
});
