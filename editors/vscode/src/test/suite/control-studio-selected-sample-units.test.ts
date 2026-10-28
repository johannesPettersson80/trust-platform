/// <reference lib="dom.iterable" />
import * as assert from "assert";
import { chartTheme } from "../../controlStudio/webview/chartPrimitives";
import { color } from "echarts/core";

suite("Control Studio selected sample announcement", () => {
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
});
