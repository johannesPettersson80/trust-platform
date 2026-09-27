import * as assert from "assert";
import {
  disposeActiveChartActionRouter,
  initializeActiveChartActionRouter,
  registerActiveChartActionTarget,
} from "../../controlStudio/webview/activeChartActionRouter";
import { executeChartToolbarAction } from "../../controlStudio/webview/chartToolbarActions";
const { JSDOM } = require("jsdom");

suite("Control Studio active chart toolbar routing", () => {
  test("routes actions to the visible engineering plot and cancels live-chart fallback", () => {
    const dom = new JSDOM(`<main><div class="plot-view" data-plot-view="response" hidden></div><div class="plot-view" data-plot-view="engineering-response"></div></main>`);
    const received: string[] = [];
    initializeActiveChartActionRouter(dom.window.document);
    const unregister = registerActiveChartActionTarget({
      plotIds: ["engineering-response"],
      handle(action: string) { received.push(action); return true; },
    }, dom.window.document);
    const event = new dom.window.CustomEvent("trust-control-shared-chart-action", { detail: { action: "zoom-in" }, cancelable: true });
    assert.strictEqual(dom.window.dispatchEvent(event), false);
    assert.deepStrictEqual(received, ["zoom-in"]);
    unregister();
  });

  test("routes only the active plot and leaves unknown or live response actions alone", () => {
    const dom = new JSDOM(`<main><div class="plot-view" data-plot-view="fit"></div><div class="plot-view" data-plot-view="general-model" hidden></div></main>`);
    const received: string[] = [];
    initializeActiveChartActionRouter(dom.window.document);
    registerActiveChartActionTarget({ plotIds: ["fit"], handle: (action: string) => { received.push(action); return true; } }, dom.window.document);
    registerActiveChartActionTarget({ plotIds: ["general-model"], handle: (action: string) => { received.push(`model:${action}`); return true; } }, dom.window.document);
    const fit = new dom.window.CustomEvent("trust-control-shared-chart-action", { detail: { action: "reset" }, cancelable: true });
    assert.strictEqual(dom.window.dispatchEvent(fit), false);
    assert.deepStrictEqual(received, ["reset"]);
    const unknown = new dom.window.CustomEvent("trust-control-shared-chart-action", { detail: { action: "table" }, cancelable: true });
    assert.strictEqual(dom.window.dispatchEvent(unknown), true);
    assert.deepStrictEqual(received, ["reset"]);
    disposeActiveChartActionRouter(dom.window.document);
  });

  test("executes inspect and zoom against the active chart data instead of the hidden live chart", () => {
    const dom = new JSDOM(`<main><div class="plot-view" data-plot-view="fit"><div data-identification-plot="fit" tabindex="0"></div></div></main>`);
    const actions: unknown[] = [];
    const chart = {
      getOption: () => ({ series: [{ data: [[0, 1], [2, 3]] }] }),
      dispatchAction: (action: unknown) => actions.push(action),
    };
    const host = dom.window.document.querySelector("[data-identification-plot=fit]") as HTMLElement;
    const windows = new Map<string, [number, number]>();
    assert.strictEqual(executeChartToolbarAction("inspect", "fit", [chart as never], host, windows), true);
    assert.deepStrictEqual(actions[0], { type: "showTip", seriesIndex: 0, dataIndex: 1 });
    assert.strictEqual(dom.window.document.activeElement, host);
    assert.strictEqual(executeChartToolbarAction("zoom-in", "fit", [chart as never], host, windows), true);
    assert.deepStrictEqual(actions[1], { type: "dataZoom", start: 12.5, end: 87.5 });
    assert.deepStrictEqual(actions[2], { type: "updateAxisPointer", currTrigger: "leave" });
    assert.strictEqual(executeChartToolbarAction("reset", "fit", [chart as never], host, windows), true);
    assert.deepStrictEqual(actions[3], { type: "dataZoom", start: 0, end: 100 });
    assert.deepStrictEqual(actions[4], { type: "updateAxisPointer", currTrigger: "leave" });
  });
});
