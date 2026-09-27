import * as assert from "assert";

import type { IdentifiedModelPresentation } from "../../controlStudio/identification";
import type * as ChartModule from "../../controlStudio/webview/engineeringCharts";

interface CapturedChart {
  options: Record<string, unknown>[];
  setOption(option: Record<string, unknown>): void;
  resize(): void;
  clear(): void;
  dispose(): void;
}

function point(timeSeconds: number, measured: number, predicted: number) {
  return { timeSeconds, measured, predicted, residual: measured - predicted };
}

function comparisonEvidence(): IdentifiedModelPresentation {
  const measured = [point(0, 35, 35), point(1, 36, 35.8), point(2, 37, 36.7)];
  const candidateA = [point(0, 35, 35), point(1, 36, 35.7), point(2, 37, 36.5)];
  const candidateB = [point(0, 35, 35), point(1, 36, 35.9), point(2, 37, 36.9)];
  const model = {
    kind: "first_order_dead_time" as const,
    gain: 1,
    timeConstantSeconds: 5,
    deadTimeSeconds: 1,
    normalizedError: .1,
    stable: true,
  };
  const fit = (points: typeof candidateA) => ({
    points,
    rootMeanSquareError: .1,
    outputUnit: "degC",
    stable: true,
    delaySeconds: 1,
    operatingRange: { inputMinimum: 75, inputMaximum: 85, outputMinimum: 35, outputMaximum: 37 },
    warnings: [],
  });
  return {
    equation: "FOPDT",
    units: { input: "%", output: "degC", gain: "degC/%", time: "s" },
    parameters: [{ name: "K", value: 1, unit: "degC/%" }],
    poles: [-.2],
    zeros: [],
    delaySeconds: 1,
    uncertainty: { label: "measurement-consistent grid" },
    estimation: {},
    validation: {},
    predictedResponse: measured,
    validityRange: { inputMinimum: 75, inputMaximum: 85, outputMinimum: 35, outputMaximum: 37 },
    warnings: [],
    // The comparison fields are supplied by the model-structure owner. Keep the
    // fixture structural so this test remains independent of fitter construction.
    modelStructure: {
      candidateA: { model, fit: fit(candidateA), maximumAbsoluteResidual: .3, finalError: .1,
        rootMeanSquareError: .1, adequate: true, analysis: {} },
      candidateB: { model, fit: fit(candidateB), maximumAbsoluteResidual: .1, finalError: .05,
        rootMeanSquareError: .05, adequate: true, analysis: {} },
      requirements: { maximumAbsoluteResidual: .2, maximumFinalError: .1 },
      rmsImprovement: .05,
      recommended: "first_order",
      physicalOrigin: "unresolved",
      provisional: true,
    },
  } as unknown as IdentifiedModelPresentation;
}

function ordinaryEvidence(): IdentifiedModelPresentation {
  const response = [point(0, 35, 35), point(1, 36, 35.8), point(2, 37, 36.7)];
  return {
    equation: "FOPDT",
    units: { input: "%", output: "degC", gain: "degC/%", time: "s" },
    parameters: [{ name: "K", value: 1, unit: "degC/%" }],
    poles: [-.2],
    zeros: [],
    delaySeconds: 1,
    uncertainty: { label: "measurement-consistent grid" },
    estimation: {},
    validation: {},
    predictedResponse: response,
    validityRange: { inputMinimum: 75, inputMaximum: 85, outputMinimum: 35, outputMaximum: 37 },
    warnings: [],
  };
}

function installChartStubs(): { charts: CapturedChart[]; restore(): void } {
  const echartsPath = require.resolve("echarts/core");
  const adapterPath = require.resolve("../../controlStudio/webview/engineeringCharts");
  const primitivesPath = require.resolve("../../controlStudio/webview/chartPrimitives");
  const originalEchartsModule = require.cache[echartsPath];
  const originalAdapterModule = require.cache[adapterPath];
  const originalPrimitivesModule = require.cache[primitivesPath];
  if (!originalEchartsModule) throw new Error("ECharts core must be loaded before chart stubbing");
  const charts: CapturedChart[] = [];
  const init = () => {
    const chart: CapturedChart = {
      options: [],
      setOption(option) { this.options.push(option); },
      resize() {}, clear() {}, dispose() {},
    };
    charts.push(chart);
    return chart;
  };
  // `echarts/core` is an ESM namespace with read-only exports. Replace only the
  // temporary CommonJS cache entry consumed by the adapter's dynamic require.
  require.cache[echartsPath] = {
    ...originalEchartsModule,
    exports: { ...originalEchartsModule.exports, init, connect: () => {} },
  } as NodeModule;
  delete require.cache[adapterPath];
  delete require.cache[primitivesPath];
  return {
    charts,
    restore: () => {
      if (originalEchartsModule) require.cache[echartsPath] = originalEchartsModule;
      else delete require.cache[echartsPath];
      if (originalAdapterModule) require.cache[adapterPath] = originalAdapterModule;
      else delete require.cache[adapterPath];
      if (originalPrimitivesModule) require.cache[primitivesPath] = originalPrimitivesModule;
      else delete require.cache[primitivesPath];
    },
  };
}

function installDom(): { restore(): void } {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main data-control-studio data-process-unit="degC" data-output-unit="%">
    <div data-identification-plot="fit"></div>
    <div data-identification-plot="residuals"></div>
    <div data-identification-plot="validation-data"></div>
    <div data-identification-plot="poles-zeros"></div>
    <p data-identification-fit-summary></p>
  </main>`);
  const globals = globalThis as unknown as Record<string, unknown>;
  const original = { document: globals.document, window: globals.window, getComputedStyle: globals.getComputedStyle,
    ResizeObserver: globals.ResizeObserver };
  globals.document = dom.window.document;
  globals.window = dom.window;
  globals.getComputedStyle = dom.window.getComputedStyle.bind(dom.window);
  globals.ResizeObserver = class { observe() {} disconnect() {} };
  return { restore: () => { Object.assign(globals, original); dom.window.close(); } };
}

suite("Control Studio model-structure plot projection", () => {
  test("projects comparison fits and residual limits, then removes them for ordinary evidence", () => {
    const dom = installDom();
    const charts = installChartStubs();
    try {
      // Import after the ECharts stub is installed so the real adapter uses captured charts.
      const { IdentificationChartAdapter } = require("../../controlStudio/webview/engineeringCharts") as typeof ChartModule;
      const adapter = new IdentificationChartAdapter(document.querySelector('[data-identification-plot="fit"]')!);
      adapter.setEvidence(comparisonEvidence());
      const fit = charts.charts[0].options.slice(-1)[0]!;
      const residuals = charts.charts[1].options.slice(-1)[0]!;
      const fitNames = (fit.series as Array<{ name: string }>).map(series => series.name);
      const residualNames = (residuals.series as Array<{ name: string }>).map(series => series.name);
      assert.deepStrictEqual(fitNames, ["Measured PV", "Candidate A", "Candidate B"]);
      assert.deepStrictEqual(residualNames, ["Candidate A residual", "Candidate B residual"]);
      const residualValues = (residuals.series as Array<{ name: string; data: Array<[number, number]> }>)
        .flatMap(series => series.data.map(point => point[1]));
      const residualAxis = residuals.yAxis as { min?: number | null; max?: number | null };
      const lowerBound = Math.min(-.2, ...residualValues), upperBound = Math.max(.2, ...residualValues);
      assert.strictEqual(typeof residualAxis.min, "number", "comparison residuals need an explicit lower axis bound");
      assert.strictEqual(typeof residualAxis.max, "number", "comparison residuals need an explicit upper axis bound");
      assert.ok(residualAxis.min! <= lowerBound, "the residual axis must contain both candidate minima and the negative limit");
      assert.ok(residualAxis.max! >= upperBound, "the residual axis must contain both candidate maxima and the positive limit");
      const guides = (residuals.series as Array<{ markLine?: { data?: Array<{ name: string; yAxis?: number }> } }>)
        .flatMap(series => series.markLine?.data ?? []);
      assert.deepStrictEqual(guides.map(guide => [guide.name, guide.yAxis]), [
        ["Residual limit", .2], ["Residual limit", -.2],
      ]);

      adapter.setEvidence(ordinaryEvidence());
      const ordinaryFit = charts.charts[0].options.slice(-1)[0]!;
      const ordinaryResiduals = charts.charts[1].options.slice(-1)[0]!;
      assert.deepStrictEqual((ordinaryFit.series as Array<{ name: string }>).map(series => series.name), ["Measured PV", "Predicted PV"]);
      assert.deepStrictEqual((ordinaryResiduals.series as Array<{ name: string }>).map(series => series.name), ["Prediction residual"]);
    } finally {
      charts.restore();
      dom.restore();
    }
  });
});
