import * as assert from "assert";
import { ControlChartAdapter } from "../../controlStudio/webview/liveChart";
import { liveSignalPlot, liveSignals } from "../../controlStudio/webview/liveSignalPlot";
import type { ControlChartSample } from "../../controlStudio/chartContract";
const echarts = require("echarts");

suite("Control Studio event marker coordinates", () => {
  test("uses actual elapsed time and clips markers to the resized and zoomed plot grids", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<div data-chart-annotations></div>');
    const original = globalThis.document;
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 1280, height: 500 });
    try {
      const samples: ControlChartSample[] = [0, 1, 9, 10].map(elapsedSeconds => ({ elapsedSeconds, sp: 20, pv: 20, mv: 0,
        quality: elapsedSeconds === 9 ? "stale" : "good", events: elapsedSeconds === 0 ? [{ kind: "mode-change", label: "Manual mode" }] : [] }));
      const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111", sp: "green", pv: "blue", mv: "red",
        auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
      const option = (width: number) => liveSignalPlot(samples, liveSignals(samples, "°C", "%"), new Set(["sp", "pv", "mv"]),
        { width, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme);
      chart.setOption({ animation: false, ...option(1280) });
      const annotations = dom.window.document.querySelector("div");
      const owner = Object.assign(Object.create(ControlChartAdapter.prototype), { chart, annotations, source: "Native heater" });
      const check = (expectedTimes: number[]) => {
        chart.renderToSVGString();
        globalThis.document = dom.window.document;
        try { owner.renderAnnotations(samples); } finally { globalThis.document = original; }
        const markers = [...annotations.children] as HTMLElement[];
        assert.strictEqual(markers.length, expectedTimes.length, "only events in the visible time window remain");
        markers.forEach((marker, index) => {
          assert.ok(Math.abs(Number.parseFloat(marker.style.left) - chart.convertToPixel({ xAxisIndex: 0 }, expectedTimes[index])) < 0.01,
            "event positions must use elapsed time and actual chart margins, not retained sample indexes");
          assert.ok(Number.parseFloat(marker.style.top) >= 60, "markers must start below the legend");
          assert.ok(Number.parseFloat(marker.style.height) < 420, "markers must finish above the time labels");
          assert.match(marker.title, new RegExp(`${expectedTimes[index]} s`));
          assert.match(marker.title, /Native heater/);
        });
        return markers;
      };
      assert.match(check([0, 9, 10])[1].title, /Data quality changed to stale/);
      chart.dispatchAction({ type: "dataZoom", start: 50, end: 100 }); check([9, 10]);
      chart.resize({ width: 640, height: 500 }); chart.setOption(option(640)); check([9, 10]);
      chart.dispatchAction({ type: "dataZoom", start: 0, end: 100 }); check([0, 9, 10]);
    } finally { chart.dispose(); globalThis.document = original; dom.window.close(); }
  });
});
