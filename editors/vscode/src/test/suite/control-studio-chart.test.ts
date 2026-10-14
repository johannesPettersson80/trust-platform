import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  CONTROL_CHART_SIGNALS,
  controlChartCsv,
  displayedControlSamples,
  frequencyPlotDescription,
  normalizeChartWindow,
  validateLogValues,
  wrapPhaseDegrees,
} from "../../controlStudio/chartContract";
import * as chartContractModule from "../../controlStudio/chartContract";
import { buildEngineeringPlotEvidence, engineeringPlotMessageEvidence } from "../../controlStudio/engineeringPlots";
import { candidateFromControlPid, type ControlPidParameters } from "../../controlStudio/controllerCandidate";
import { responseChartSeries, responseChartTooltip } from "../../controlStudio/webview/threePageResponseSeries";
import { rootLocusChartSeries } from "../../controlStudio/webview/rootLocusChart";
import { sensitivitySeries } from "../../controlStudio/webview/sensitivityChart";
import { nyquistGeometry, nyquistSeries } from "../../controlStudio/webview/nyquistChart";
import { initialStateResponseRows, initialStateResponseSeries } from "../../controlStudio/webview/initialStateResponseChart";
import { poleMapGeometry } from "../../controlStudio/webview/complexPlaneGeometry";

suite("Control Studio chart contract", () => {
  test("a provisional structure comparison exposes paired fit, residuals and the original record without claiming validation data", () => {
    const tabs = chartContractModule.controlStagePlots("Identify", { recording: true, model: true, structureComparison: true } as any);
    assert.deepStrictEqual(tabs.map(tab => tab.id), ["fit", "residuals", "recorded-response"]);
    assert.ok(tabs.every(tab => tab.available));
  });
  test("Identify exposes the retained response before fitting and keeps preview and recording views distinct", () => {
    const before = chartContractModule.controlStagePlots("Identify", { recording: true });
    assert.ok(before.some(plot => plot.id === "recorded-response" && plot.available),
      "the selected recording must be inspectable before its model exists");
    assert.ok(before.some(plot => plot.id === "fit" && !plot.available && !/accept/i.test(plot.reason ?? "")),
      "a first fit must not tell the engineer to accept a nonexistent model");
    assert.strictEqual(chartContractModule.controlPlotView("Identify", "recorded-response"), "response");
    const after = chartContractModule.controlStagePlots("Identify", { recording: true, model: true });
    assert.ok(after.some(plot => plot.id === "fit" && plot.available));
    assert.ok(after.some(plot => plot.id === "recorded-response" && plot.available));
  });
  const threePageTheme = { foreground: "black", muted: "gray", border: "gray", background: "white",
    sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "gold" };
  const threePageLoop = () => {
    const signal = (symbol: string) => ({ symbol, address: "%ML0", iecType: "LREAL", direction: "read" as const,
      unit: "%", range: { minimum: 0, maximum: 100 } });
    return { signals: { setpoint: signal("P1.SP"), processVariable: signal("P1.PV"), manipulatedVariable: signal("P1.MV"),
      additional: Array.from({ length: 9 }, (_, index) => ({ ...signal(`P1.State${index}`), id: `state-${index}`,
        role: "state", channel: `state-${index}` })) } } as any;
  };
  const threePagePlot = (loop: any, ids: string[], prediction = false) => responseChartSeries(threePageTheme, loop,
    [{ name: "Response", unit: "%", series: ids.map(signalId => ({ signalId, name: signalId, dashed: signalId === "sp",
      prediction, points: [[0, 1]] })) }]);
  const threePageIdentity = (series: ReturnType<typeof threePagePlot>[number]) =>
    `${(series.lineStyle as { color?: string }).color}/${series.lineStyle.type}`;
  test("the initial-state response draws the total as the wide primary line with its two parts over it in distinct auxiliary strokes", () => {
    const points = [{ timeSeconds: 0, zeroState: [9, 0], initialCondition: [9, 0.02], total: [9, 0.02] }, { timeSeconds: 1, zeroState: [9, 0.001], initialCondition: [9, 0.019], total: [9, 0.02] }];
    const colors = { total: "#0a84ff", zeroState: "#ff9f0a", initialCondition: "#bf5af2" };
    const series = initialStateResponseSeries(points, 1, colors) as any[];
    assert.deepStrictEqual(series.map(item => item.name), ["Zero-state input", "Initial condition", "Total"]);
    assert.deepStrictEqual(series.map(item => item.data[1][1]), [0.001, 0.019, 0.02]);
    assert.deepStrictEqual(series.map(item => item.lineStyle.color), [colors.zeroState, colors.initialCondition, colors.total]);
    assert.deepStrictEqual(series.map(item => item.itemStyle.color), [colors.zeroState, colors.initialCondition, colors.total]);
    assert.deepStrictEqual(series.map(item => item.lineStyle.type), ["dashed", "dotted", "solid"]);
    const [zeroState, initialCondition, total] = series;
    assert.ok(total.lineStyle.width > zeroState.lineStyle.width && total.lineStyle.width > initialCondition.lineStyle.width, "The total is the wide primary line");
    assert.ok(zeroState.z > total.z && initialCondition.z > total.z, "A part that coincides with the total stays visible on top of it");
  });

  test("the initial-state response table shows each column in one notation", () => {
    const points = [
      { timeSeconds: 0, zeroState: [0], initialCondition: [0], total: [0] },
      { timeSeconds: 1, zeroState: [3.89e-5], initialCondition: [7.69e-4], total: [8.08e-4] },
      { timeSeconds: 2, zeroState: [1.52e-4], initialCondition: [0.001477], total: [0.001629] },
      { timeSeconds: 120, zeroState: [0.0476], initialCondition: [7.9e-4], total: [0.0484] },
    ];
    assert.deepStrictEqual(initialStateResponseRows(points, 0), [
      ["0", "0", "0", "0"],
      ["1", "0.000039", "0.000769", "0.000808"],
      ["2", "0.000152", "0.001477", "0.001629"],
      ["120", "0.0476", "0.00079", "0.0484"],
    ]);
  });

  test("the three-page response plot keeps the primary strokes and each auxiliary identity across selections", () => {
    const loop = threePageLoop();
    const ids = ["sp", "pv", "mv", ...loop.signals.additional.map((signal: { id: string }) => signal.id)];
    const all = threePagePlot(loop, ids);
    assert.deepStrictEqual(all.slice(0, 3).map(threePageIdentity), ["green/dashed", "blue/solid", "red/solid"]);
    assert.deepStrictEqual(threePagePlot(loop, ["pv", "state-4"]).map(threePageIdentity), [all[1], all[7]].map(threePageIdentity),
      "signal selection must not reassign the remaining trace identities");
    assert.strictEqual((threePagePlot(loop, ["pv"], true)[0].lineStyle as { color?: string }).color, "orange",
      "a model prediction keeps its prediction stroke");
  });
  test("the three-page response plot never draws two displayed signals alike", () => {
    const loop = threePageLoop();
    const ids = ["sp", "pv", "mv", ...loop.signals.additional.map((signal: { id: string }) => signal.id)];
    const all = threePagePlot(loop, ids);
    assert.strictEqual(new Set(all.map(threePageIdentity)).size, ids.length,
      `each displayed trace needs its own colour/style pair: ${all.map(threePageIdentity).join(", ")}`);
  });
  test("the three-page response plot draws the applied output as held between samples", () => {
    const loop = threePageLoop();
    const [pv, mv] = threePagePlot(loop, ["pv", "mv"]);
    assert.strictEqual((mv as { step?: unknown }).step, "end", "the actuator keeps each command until the next update");
    assert.strictEqual((pv as { step?: unknown }).step, undefined, "a measurement is joined as sampled");
  });
  test("the three-page response tooltip shows engineering values inside the plot", () => {
    const tooltip = responseChartTooltip();
    const shown = (value: number) => (tooltip.valueFormatter ?? String)(value);
    assert.strictEqual(tooltip.trigger, "axis");
    assert.strictEqual(shown(1.9017502305516345), "1.9", "a recorded torque reads like the other engineering charts, not as raw binary digits");
    assert.strictEqual(shown(-2.405530441441469), "-2.41");
    assert.strictEqual(tooltip.confine, true, "the tooltip stays inside the plot area");
  });
  test("the three-page root locus draws the pole paths, labels the inspected gains and marks the selected candidate", () => {
    // The engine's root-locus answer for the shaft (RootLocusAnswer, analysis/declared.rs): the poles of s² + 8s + Kp = 0 at each
    // gain, the three swept gains and the 201 evenly spaced gains of the declared range over which the pole paths are drawn.
    const shaftPoles = (gain: number) => {
      const root = Math.sqrt(16 - gain);
      return root >= 0
        ? [{ real: -4 + root, imaginary: 0 }, { real: -4 - root, imaginary: 0 }]
        : [{ real: -4, imaginary: Math.sqrt(gain - 16) }, { real: -4, imaginary: -Math.sqrt(gain - 16) }];
    };
    const result = {
      parameter: "Kp", parameterUnit: "N·m/rad", selectedValue: 16,
      fixedParameters: { Kd: { value: 7, unit: "N·m·s/rad" } },
      provenance: "provided model shaft-kd7 · unvalidated",
      approximation: "Declared continuous linear model; nonlinear limits and task sample/hold are not represented. No model delay.",
      branches: [12, 16, 20].map(gain => ({ gain, poles: shaftPoles(gain) })),
      path: Array.from({ length: 201 }, (_, index) => ({ gain: index / 10, poles: shaftPoles(index / 10) })),
    };
    const colors = { path: "#0000ff", inspected: "#ff8800", selected: "#aa00ff", foreground: "#ffffff" };
    const series = rootLocusChartSeries(result, colors) as any[];
    const paths = series.find(item => item.type === "line");
    assert.ok(paths, "one line series joins the poles of consecutive gains");
    assert.match(paths.name, /Kp 0–20 N·m\/rad/);
    assert.strictEqual(paths.lineStyle.color, colors.path);
    const breaks = paths.data.filter((point: unknown[]) => point[0] === null).length;
    assert.strictEqual(breaks, 1, "the two branches are separate paths");
    assert.strictEqual(paths.data.length, 2 * 201 + 1);
    for (const start of [[-8, 0], [0, 0]]) assert.ok(paths.data.some((point: number[]) => point[0] !== null
      && Math.abs(point[0] - start[0]) < 1e-9 && Math.abs(point[1] - start[1]) < 1e-9), `the path starts at ${start}`);
    const inspected = series.find(item => item.name === "Inspected gains");
    assert.deepStrictEqual(inspected.data.map((point: { value: number[] }) => point.value[2]), [12, 12, 20, 20]);
    assert.strictEqual(inspected.itemStyle.color, colors.inspected);
    assert.strictEqual(inspected.label.formatter({ value: inspected.data[0].value }), "Kp = 12");
    const selected = series.find(item => item.name === "Selected Kp = 16");
    assert.ok(selected, "the selected candidate is its own legend entry");
    assert.strictEqual(selected.symbol, "diamond");
    assert.ok(selected.symbolSize > inspected.symbolSize);
    assert.strictEqual(selected.itemStyle.color, colors.selected);
    assert.deepStrictEqual(selected.data.map((point: { value: number[] }) => point.value.map(value => Math.round(value * 1e6) / 1e6)), [[-4, 0, 16], [-4, 0, 16]]);
    // Labels never sit on a path: above and right of a pole on the real axis, where a branch may also leave
    // vertically, and beside a pole off it.
    const aboveRight = [8, -18];
    assert.deepStrictEqual(inspected.data.map((point: { label: { position: unknown } }) => point.label.position), [aboveRight, aboveRight, "right", "right"]);
    assert.deepStrictEqual(selected.data.map((point: { label: { position: unknown } }) => point.label.position), [aboveRight, aboveRight]);
  });

  test("the sensitivity view gives S, T and CS their own visible colours and strokes", () => {
    const points = [{ frequencyRadPerSecond: 0.2, sensitivityMagnitudeDb: -2.05, complementarySensitivityMagnitudeDb: -2.05, controlSensitivityMagnitudeDb: 14.94 }];
    const colors = { sensitivity: "#0000ff", complementary: "#ff8800", control: "#aa00ff" };
    const series = sensitivitySeries(points, colors) as any[];
    assert.deepStrictEqual(series.map(item => item.name), ["S", "T", "CS"]);
    assert.deepStrictEqual(series.map(item => item.lineStyle.color), [colors.sensitivity, colors.complementary, colors.control]);
    assert.deepStrictEqual(series.map(item => item.itemStyle.color), [colors.sensitivity, colors.complementary, colors.control]);
    assert.deepStrictEqual(series.map(item => item.lineStyle.type), ["solid", "dashed", "dotted"]);
    assert.deepStrictEqual(series.map(item => item.data[0][1]), [-2.05, -2.05, 14.94]);
  });

  // L(jω) = 2·e^(−8jω)/(1 + 5jω): the heater under P control with an 8 s delivery delay.
  const delayedLoop = Array.from({ length: 400 }, (_, index) => {
    const omega = 0.01 * Math.pow(4000, index / 399);
    const scale = 2 / (1 + 25 * omega * omega);
    const c = Math.cos(8 * omega), q = Math.sin(8 * omega);
    return { real: scale * (c - 5 * omega * q), imaginary: -scale * (q + 5 * omega * c) };
  });


  test("the Nyquist plot keeps equal units on both axes around the curve and −1", () => {
    for (const area of [{ width: 1250, height: 440 }, { width: 500, height: 440 }]) {
      const geometry = nyquistGeometry(delayedLoop, area);
      const { grid, x, y } = geometry;
      assert.ok(x && y && grid.width && grid.height, `explicit axis ranges and plot size: ${JSON.stringify(geometry)}`);
      const perPixel = [(x.max - x.min) / grid.width, (y.max - y.min) / grid.height];
      assert.ok(Math.abs(perPixel[0] / perPixel[1] - 1) < 1e-9, `equal units per pixel: ${perPixel}`);
      assert.ok(grid.left >= 0 && grid.top >= 0 && grid.left + grid.width <= area.width && grid.top + grid.height <= area.height, `inside the plot area: ${JSON.stringify(grid)}`);
      for (const point of [...delayedLoop, { real: -1, imaginary: 0 }]) {
        assert.ok(point.real >= x.min && point.real <= x.max && -Math.abs(point.imaginary) >= y.min && Math.abs(point.imaginary) <= y.max, `point and its mirror inside: ${JSON.stringify(point)}`);
      }
    }
  });

  test("the Nyquist plot marks −1 and shows the traversal direction on both branches", () => {
    const colors = { positive: "#3b82f6", negative: "#eab308", critical: "#ef4444", label: "#cccccc" };
    const series = nyquistSeries(delayedLoop, colors) as any[];
    const critical = series.find(item => item.name === "−1");
    assert.deepStrictEqual(critical?.data, [[-1, 0]]);
    assert.strictEqual(critical.itemStyle?.color, colors.critical);
    assert.strictEqual(critical.label?.show, true);
    const arrows = (name: string) => series.filter(item => item.name === name && item.symbol === "arrow").flatMap(item => item.data);
    const rotation = (from: number[], to: number[]) => Math.atan2(to[1] - from[1], to[0] - from[0]) * 180 / Math.PI - 90;
    const positive = arrows("Positive frequency");
    const negative = arrows("Negative frequency");
    assert.ok(positive.length >= 3 && negative.length >= 3, `arrows on both branches: ${positive.length}, ${negative.length}`);
    for (const arrow of positive) {
      const index = delayedLoop.findIndex(point => point.real === arrow.value[0] && point.imaginary === arrow.value[1]);
      assert.ok(index >= 0 && index < delayedLoop.length - 1, "a positive-branch arrow sits on the curve");
      const [from, to] = [delayedLoop[index], delayedLoop[index + 1]].map(point => [point.real, point.imaginary]);
      assert.ok(Math.abs(arrow.symbolRotate - rotation(from, to)) < 1e-9, "the positive branch runs with rising frequency");
    }
    for (const arrow of negative) {
      const index = delayedLoop.findIndex(point => point.real === arrow.value[0] && -point.imaginary === arrow.value[1]);
      assert.ok(index >= 0 && index < delayedLoop.length - 1, "a negative-branch arrow sits on the mirrored curve");
      const [from, to] = [delayedLoop[index + 1], delayedLoop[index]].map(point => [point.real, -point.imaginary]);
      assert.ok(Math.abs(arrow.symbolRotate - rotation(from, to)) < 1e-9, "the negative branch runs from high frequency back to zero");
    }
  });

  test("the pole-zero map keeps equal units on both axes and shows the whole stability boundary", () => {
    // The sampled shaft loop at h = 0.6 s: a complex pair outside the unit circle, a real pole and a sampling zero inside.
    const poles = [{ real: 0.6704, imaginary: 2.3151 }, { real: 0.6704, imaginary: -2.3151 }, { real: 0.2079, imaginary: 0 }];
    const zeros = [{ real: -0.8192, imaginary: 0 }];
    const cases = [
      { discrete: true, boundary: [{ real: 1, imaginary: 0 }, { real: -1, imaginary: 0 }, { real: 0, imaginary: 1 }, { real: 0, imaginary: -1 }] },
      { discrete: false, boundary: [{ real: 0, imaginary: 0 }] },
    ];
    for (const { discrete, boundary } of cases) for (const area of [{ width: 1250, height: 440 }, { width: 500, height: 440 }]) {
      const geometry = poleMapGeometry(poles, zeros, discrete, area);
      const { grid, x, y } = geometry;
      assert.ok(x && y && grid.width && grid.height, `explicit axis ranges and plot size: ${JSON.stringify(geometry)}`);
      const perPixel = [(x.max - x.min) / grid.width, (y.max - y.min) / grid.height];
      assert.ok(Math.abs(perPixel[0] / perPixel[1] - 1) < 1e-9, `equal units per pixel: ${perPixel}`);
      for (const point of [...poles, ...zeros, ...boundary]) {
        assert.ok(point.real >= x.min && point.real <= x.max && point.imaginary >= y.min && point.imaginary <= y.max, `inside: ${JSON.stringify(point)}`);
      }
    }
  });

  test("retains auxiliary identities and independent quality when reopening measured samples", () => {
    const sample = { elapsedSeconds: 1, sp: 45, pv: 39, appliedMv: 100,
      quality: "good" as const, events: [], additional: [
        { id: "requested-power", channel: "heater", role: "state" as const, unit: "%", value: 125, quality: "good" as const },
        { id: "stored-correction", channel: "heater", role: "state" as const, unit: "%", value: 113, quality: "forced" as const },
      ] };
    assert.deepStrictEqual(chartContractModule.restoredControlChartSamples([sample]), [{
      elapsedSeconds: 1, sp: 45, pv: 39, mv: 100, quality: "good", events: [],
      additional: sample.additional,
    }], "saved auxiliary values, units and quality must reach the shared plot without replacing applied MV");
  });
  test("retains auxiliary extrema and quality gaps during display reduction", () => {
    const samples = Array.from({ length: 21 }, (_, index) => ({
      elapsedSeconds: index, sp: 45, pv: 39, mv: 100,
      additional: [{ id: "request", channel: "heater", role: "state" as const, unit: "%",
        value: index === 3 ? 125 : index === 11 ? null : 100,
        quality: index === 7 ? "bad" as const : "good" as const }],
    }));
    const displayed = displayedControlSamples(samples, 3);
    for (const index of [3, 7, 8, 11, 12]) assert.ok(displayed.some(sample => sample.elapsedSeconds === index),
      `auxiliary extremum or quality/gap boundary ${index} must not disappear`);
  });
  test("exports auxiliary samples with unit and quality alongside primary measured values", () => {
    const csv = controlChartCsv([{ elapsedSeconds: 1, sp: 45, pv: 39, mv: 100,
      additional: [{ id: "request", channel: "heater", role: "state", unit: "%", value: 125, quality: "good" }],
    } as Parameters<typeof controlChartCsv>[0][number]]);
    assert.ok(csv.includes('"request (%)"'), "export must name the additional signal and its unit");
    assert.ok(csv.includes('"request.quality"'), "export must retain additional quality");
    assert.ok(csv.includes(',125,good'), "export must retain the unclipped request");
  });
  test("opens recording-only validation evidence on its measured response without requiring a candidate", () => {
    const tabs = chartContractModule.controlStagePlots("Validate", { recording: true });
    const selected = chartContractModule.selectControlPlot(tabs, "requirements");
    assert.strictEqual(selected, "recorded-response");
    assert.strictEqual(chartContractModule.controlPlotView("Validate", selected!), "response");
    assert.deepStrictEqual(tabs.map(tab => tab.id), ["recorded-response", "signals"],
      "a recording must not expose unavailable candidate-only plots");
    assert.strictEqual(chartContractModule.controlStagePlots("Validate", {}).some(tab => tab.id === "recorded-response"), false);
    assert.strictEqual(chartContractModule.selectControlPlot(
      chartContractModule.controlStagePlots("Validate", { recording: true, candidate: true }), "requirements"), "requirements",
    "qualified saved analysis keeps its selected view");
  });
  test("offers explicitly live validation inspection without manufacturing recorded evidence", () => {
    const tabs = chartContractModule.controlStagePlots("Validate", { samples: true });
    assert.deepStrictEqual(tabs.map(tab => tab.id), ["live-response"]);
    assert.strictEqual(chartContractModule.selectControlPlot(tabs, "requirements"), "live-response");
    assert.strictEqual(chartContractModule.controlPlotView("Validate", "live-response"), "response");
    assert.ok(!tabs.some(tab => tab.id === "recorded-response"));
  });
  test("manual parameter tuning shows the measured response without an invented model prediction", () => {
    const evidence = { samples: true, manualParameters: true };
    assert.deepStrictEqual(chartContractModule.controlStagePlots("Tune", evidence).map(tab => tab.id), ["live-response"]);
    assert.deepStrictEqual(chartContractModule.controlStagePlots("Tune", { ...evidence, recording: true }).map(tab => tab.id), ["recorded-response"]);
    const modelTabs = chartContractModule.controlStagePlots("Tune", { ...evidence, model: true, candidate: true });
    assert.ok(modelTabs.some(tab => tab.id === "bode" && tab.available));
    assert.strictEqual(chartContractModule.controlPlotView("Tune", "live-response"), "response");
  });

  test("formats engineering values compactly for bounded metric cards", () => {
    const format = (chartContractModule as unknown as {
      formatEngineeringNumber?: (value: unknown, fractionDigits?: number) => string;
    }).formatEngineeringNumber;
    assert.strictEqual(typeof format, "function", "compact engineering-number formatting is not implemented");
    assert.strictEqual(format?.(1_234.56), "1235");
    assert.strictEqual(format?.(104.36086038284864), "104.4");
    assert.strictEqual(format?.(15.188151194353311), "15.19");
    assert.strictEqual(format?.(-0.004321), "-0.004321");
    assert.strictEqual(format?.(0.00000012), "1.20e-7");
    assert.strictEqual(format?.(0.123456, 4), "0.1235");
    assert.strictEqual(format?.(undefined), "—");
    assert.strictEqual(format?.(Number.POSITIVE_INFINITY), "∞");
    assert.strictEqual(format?.("unbounded"), "∞");
    assert.strictEqual(format?.(Number.NEGATIVE_INFINITY), "−∞");
    assert.strictEqual(format?.(Number.NaN), "—");
  });

  test("keeps revision identity available without filling the primary header", () => {
    const compact = (chartContractModule as unknown as {
      compactRevisionLabel?: (value: string) => string;
    }).compactRevisionLabel;
    assert.strictEqual(typeof compact, "function", "compact revision labels are not implemented");
    assert.strictEqual(compact?.("sha256:1234567890abcdef"), "12345678…");
    assert.strictEqual(compact?.("controller-2"), "controller-2");
  });

  test("uses a compact readable plot layout at 200 percent zoom", () => {
    const layout = (chartContractModule as unknown as {
      controlChartLayout?: (width: number) => unknown;
    }).controlChartLayout;
    assert.strictEqual(typeof layout, "function", "responsive Control Studio plot layout is not implemented");
    assert.deepStrictEqual(layout?.(346), {
      compact: true, grid: { left: 36, right: 36, top: 18, bottom: 34 },
      showLegend: false, showAxisNames: false,
    });
    assert.deepStrictEqual(layout?.(900), {
      compact: false, grid: { left: 58, right: 58, top: 38, bottom: 42 },
      showLegend: true, showAxisNames: true,
    });
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const styles = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageStyles.ts"), "utf8");
    assert.match(template, /data-studio-three-page/);
    assert.match(template, /class="studio-response"[^>]*aria-label="Response plot"/);
    assert.match(styles, /\.studio-chart \{ width: 100%; height: max\(280px/,
      "the response plot must retain a bounded readable height");
    assert.match(styles, /@media\(max-width: 650px\).*\.studio-chart \{ height: 320px; \}/s,
      "narrow layouts must preserve a usable chart height");
    assert.doesNotMatch(styles, /overflow-x: clip/,
      "the current shell must keep the viewport available to its own scroll owner");
  });

  test("keeps the approved response plot readable without legacy plot-editor controls", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const styles = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageStyles.ts"), "utf8");
    assert.match(template, /class="studio-response"/);
    assert.match(template, /data-control-studio-chart/);
    assert.match(styles, /\.studio-chart \{ width: 100%; height: max\(280px/);
    assert.match(styles, /@media\(max-width: 650px\).*\.studio-chart \{ height: 320px; \}/s);
    assert.match(styles, /data-binding-draft="true"/);
    assert.doesNotMatch(template, /plot-toolbar|plot-tools|data-design-exact|data-engineering-bode/,
      "the approved three-page shell does not expose legacy plot-editor controls");
  });

  test("disables direct chart actions when the selected plot has no evidence", () => {
    const availability = (chartContractModule as unknown as {
      directChartActionAvailability?: (plotAvailable: boolean) => Record<string, boolean>;
    }).directChartActionAvailability;
    assert.strictEqual(typeof availability, "function",
      "direct chart actions need one availability contract owned by the selected plot");
    assert.deepStrictEqual(availability?.(true), {
      inspect: true, "zoom-out": true, "zoom-in": true,
      reset: true, expand: true, "export-figure": true,
    });
    assert.deepStrictEqual(availability?.(false), {
      inspect: false, "zoom-out": false, "zoom-in": false,
      reset: false, expand: false, "export-figure": false,
    });
  });

  test("does not present an empty saved-result history as an operation failure", () => {
    const status = (chartContractModule as unknown as {
      workflowResultStatusText?: (phase: string, reason?: unknown) => string | undefined;
    }).workflowResultStatusText;
    assert.strictEqual(typeof status, "function",
      "saved-result history messages need an explicit neutral-versus-failure contract");
    assert.strictEqual(status?.("history"), undefined,
      "an empty history must preserve the neutral Validate empty state");
    assert.strictEqual(status?.("failed", "Saved result is malformed."), "Saved result is malformed.");
    assert.strictEqual(status?.("failed"), "The workflow result operation failed.");
  });

  test("keeps the current three-page actions gated until their prerequisite evidence exists", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /data-engineer-find[^>]* disabled/,
      "Identify must remain gated when no declared loop is bound");
    assert.match(template, /data-engineer-tune disabled/,
      "Tune must remain gated until an identified model is available");
    assert.match(template, /data-engineer-verify disabled/,
      "Validate must remain gated until a candidate exists");
    assert.match(template, /data-engineer-apply disabled/,
      "Apply must remain gated until validation has passed");
    assert.match(workflow, /operation\.synchronize\(\); application\.synchronize\(\);/,
      "current Operation/Application gates must be synchronized after host updates");
  });

  test("derives controller-setting units from the bound process and output signals", () => {
    const deriveUnits = (chartContractModule as unknown as {
      pidEngineeringUnits?: (processUnit: string, outputUnit: string) => unknown;
    }).pidEngineeringUnits;
    assert.strictEqual(typeof deriveUnits, "function", "controller-setting unit derivation is not implemented");
    assert.deepStrictEqual(deriveUnits?.("°C", "%"), {
      proportional: "%/°C",
      integral: "%/(°C·s)",
      derivative: "%·s/°C",
      output: "%",
    });
    assert.deepStrictEqual(deriveUnits?.("degC", "%"), {
      proportional: "%/°C",
      integral: "%/(°C·s)",
      derivative: "%·s/°C",
      output: "%",
    });
    assert.deepStrictEqual(deriveUnits?.("", ""), {
      proportional: "normalized output/process",
      integral: "normalized output/(process·s)",
      derivative: "normalized output·s/process",
      output: "normalized output",
    });
  });

  test("summarizes the current plot with source, signals, values, and units", () => {
    const summary = (chartContractModule as unknown as {
      controlChartAccessibleSummary?: (
        source: string, sample: { elapsedSeconds: number; sp: number; pv: number; mv: number },
        processUnit: string, outputUnit: string
      ) => string;
    }).controlChartAccessibleSummary;
    assert.strictEqual(typeof summary, "function", "accessible live-chart summary is not implemented");
    assert.strictEqual(summary?.("Simulation", { elapsedSeconds: 4.25, sp: 5, pv: 4.3821, mv: 3.2678 }, "°C", "%"),
      "Simulation control-loop time plot. Current at 4.25 s: SP 5 °C, PV 4.38 °C, MV 3.27 %.");
  });

  test("describes explicit open-loop values accessibly without a fictitious setpoint", () => {
    const summary = (chartContractModule.controlChartAccessibleSummary as (...args: any[]) => string)(
      "Simulation", { elapsedSeconds: 2, sp: null, pv: 35, mv: 85 }, "degC", "%", "open-loop");
    assert.strictEqual(summary, "Simulation open-loop time plot. Current at 2 s: Measured output 35 °C, Applied input 85 %.");
    const axes = (chartContractModule.controlChartAxisLabels as (...args: any[]) => unknown)("degC", "%", "open-loop");
    assert.deepStrictEqual(axes, { process: "Measured output (°C)", output: "Applied input (%)",
      description: "Measured output in °C; applied input in %" });
  });

  test("projects retained experiment and quality events in chronological order", () => {
    const rows = (chartContractModule as unknown as {
      controlChartEventRows?: (samples: readonly unknown[]) => unknown[];
    }).controlChartEventRows;
    assert.strictEqual(typeof rows, "function", "the Events plot has no retained-event projection");
    assert.deepStrictEqual(rows?.([
      { elapsedSeconds: 0, sp: 30, pv: 30, mv: 20, quality: "good",
        events: [{ kind: "test-start", label: "Experiment approved" }] },
      { elapsedSeconds: 5, sp: 30, pv: 30.4, mv: 30, quality: "uncertain" },
      { elapsedSeconds: 40, sp: 30, pv: 32, mv: 20, quality: "good",
        events: [{ kind: "test-end", label: "Restoration confirmed" }] },
    ]), [
      { timeSeconds: 0, kind: "test-start", label: "Experiment approved" },
      { timeSeconds: 5, kind: "quality", label: "Data quality changed to uncertain" },
      { timeSeconds: 40, kind: "test-end", label: "Restoration confirmed" },
      { timeSeconds: 40, kind: "quality", label: "Data quality changed to good" },
    ]);
    assert.deepStrictEqual(rows?.([
      { elapsedSeconds: 0, sp: 30, pv: 30, mv: 20, quality: "good",
        events: [{ kind: "event", label: "recording_started" }] },
    ]), [{ timeSeconds: 0, kind: "event", label: "Recording started" }],
    "stored event identifiers must be presented as readable language");
  });

  test("keeps the current three-page plot owner separate from legacy Bode editor markup", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    assert.match(template, /data-control-studio-chart/);
    assert.match(template, /aria-label="Response plot"/);
    assert.doesNotMatch(template, /bode-grid|plot-toolbar|analysis-grid/,
      "the current three-page panel must not retain obsolete Bode/editor containers");
  });

  test("keeps the current shell free of obsolete graphical gain controls", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    assert.doesNotMatch(template, /data-design-exact|data-design-error|data-design-handle/);
    assert.match(template, /data-engineer-tuning-method/);
    assert.match(template, /data-studio-tuning-result/);
  });

  test("keeps exact graphical gain parsing as a pure controller input boundary", () => {
    const parseGraphicalGain = (chartContractModule as unknown as {
      parseGraphicalGain?: (text: string, minimum: number, maximum: number) => unknown;
    }).parseGraphicalGain;
    assert.strictEqual(typeof parseGraphicalGain, "function", "exact graphical gain validation is not implemented");
    assert.deepStrictEqual(parseGraphicalGain?.("4.25", 0.01, 20), { ok: true, value: 4.25 });
    assert.deepStrictEqual(parseGraphicalGain?.("0", 0.01, 20), {
      ok: false, reason: "Enter a finite value from 0.01 to 20.",
    });
    assert.deepStrictEqual(parseGraphicalGain?.("not-a-number", 0.01, 20), {
      ok: false, reason: "Enter a finite value from 0.01 to 20.",
    });
  });

  test("keeps candidate settings in the Tune result area without legacy margin cards", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    assert.match(template, /data-studio-tuning-result/);
    assert.doesNotMatch(template, /data-design-gain-margin|data-design-phase-margin/);
  });

  test("keeps stable SP, PV, and MV identity without relying on color", () => {
    assert.deepStrictEqual(CONTROL_CHART_SIGNALS.map(({ id, name, axis }) => ({ id, name, axis })), [
      { id: "sp", name: "SP", axis: 0 },
      { id: "pv", name: "PV", axis: 0 },
      { id: "mv", name: "MV", axis: 1 },
      { id: "disturbance", name: "Disturbance", axis: 1 },
      { id: "current", name: "Current", axis: 0 },
      { id: "candidate", name: "Candidate", axis: 0 },
      { id: "baseline", name: "Baseline", axis: 0 },
      { id: "limit", name: "Limit", axis: 0 },
    ]);
    assert.deepStrictEqual(CONTROL_CHART_SIGNALS.map((signal) => signal.line), [
      // SP is dashed (not solid) so it stays visible drawn on top of a coinciding solid
      // PV trace instead of being hidden underneath it (finding V-08).
      "dashed",
      "solid",
      "solid",
      "dotted",
      "solid",
      "solid",
      "dashed",
      [8, 4, 2, 4]
    ]);
    assert.deepStrictEqual(CONTROL_CHART_SIGNALS.slice(0, 3).map((signal) => signal.symbol), [
      "diamond", "circle", "rect"
    ]);
  });

  test("clamps zoom and pan to a valid visible window", () => {
    assert.deepStrictEqual(normalizeChartWindow(-20, 30), [0, 50]);
    assert.deepStrictEqual(normalizeChartWindow(80, 130), [50, 100]);
    assert.deepStrictEqual(normalizeChartWindow(50, 50), [50, 51]);
  });

  test("exports missing values as gaps instead of invented samples", () => {
    assert.strictEqual(
      controlChartCsv([{ elapsedSeconds: 1.5, sp: 10, pv: null, mv: 42 }]),
      "elapsed_seconds,sp,pv,mv,quality,events\n1.5,10,,42,good,\"\""
    );
  });

  test("restores the saved raw recording into the shared control plot", () => {
    const restore = (chartContractModule as unknown as {
      restoredControlChartSamples?: (samples: readonly unknown[]) => unknown;
    }).restoredControlChartSamples;
    assert.strictEqual(typeof restore, "function", "saved-result chart restoration is not implemented");
    assert.deepStrictEqual(restore?.([{
      elapsedSeconds: 1.25, sp: 5, pv: 4.2, mv: 12, quality: "forced",
      events: ["recording_started", "setpoint_step"],
    }]), [{
      elapsedSeconds: 1.25, sp: 5, pv: 4.2, mv: 12, quality: "bad",
      events: [{ kind: "event", label: "recording_started" }, { kind: "event", label: "setpoint_step" }],
    }]);
    assert.deepStrictEqual(restore?.([{
      elapsedSeconds: 2, sp: 5, pv: 4.4, requestedMv: null, appliedMv: 12.5,
      quality: "good", events: [],
    }]), [{ elapsedSeconds: 2, sp: 5, pv: 4.4, mv: 12.5, quality: "good", events: [] }],
    "schema-v3 recordings must plot the applied actuator output, not the optional request");
  });

  test("preserves gaps, extrema, quality changes, and events when reducing display data", () => {
    const samples = [
      { elapsedSeconds: 0, sp: 0, pv: 0, mv: 0, quality: "good" as const },
      { elapsedSeconds: 1, sp: 1, pv: 8, mv: 1, quality: "good" as const },
      { elapsedSeconds: 2, sp: 2, pv: null, mv: 2, quality: "stale" as const },
      { elapsedSeconds: 3, sp: 3, pv: 9, mv: 3, quality: "good" as const, events: [{ kind: "mode-change" as const, label: "Auto" }] },
      { elapsedSeconds: 4, sp: 4, pv: 1, mv: 4, quality: "good" as const },
      { elapsedSeconds: 5, sp: 5, pv: 7, mv: 5, quality: "good" as const },
    ];
    const displayed = displayedControlSamples(samples, 3);
    assert.ok(displayed.some((sample) => sample.pv === null));
    assert.ok(displayed.some((sample) => sample.pv === 9));
    assert.ok(displayed.some((sample) => sample.events?.[0].label === "Auto"));
    assert.deepStrictEqual([
      displayed[0].elapsedSeconds,
      displayed[displayed.length - 1].elapsedSeconds,
    ], [0, 5]);
  });

  test("rejects invalid logarithmic values and labels frequency conventions", () => {
    assert.deepStrictEqual(validateLogValues([1, 0, -1, Number.NaN, 10]), {
      values: [1, null, null, null, 10],
      invalidIndices: [1, 2, 3],
    });
    assert.strictEqual(wrapPhaseDegrees(270, "-180..180"), -90);
    assert.strictEqual(wrapPhaseDegrees(-90, "0..360"), 270);
    assert.strictEqual(
      frequencyPlotDescription("rad/s", "dB", "estimated"),
      "Estimated frequency response; frequency in rad/s; amplitude in dB; phase in degrees"
    );
  });

  test("exposes every stage plot directly and explains unavailable evidence", () => {
    const plots = (chartContractModule as unknown as {
      controlStagePlots?: (stage: string, evidence: Record<string, boolean>) =>
        Array<{ id: string; available: boolean; reason?: string }>;
    }).controlStagePlots;
    assert.strictEqual(typeof plots, "function", "direct stage plot navigation is not implemented");
    assert.deepStrictEqual(plots?.("Monitor", { samples: false }), [
      { id: "response", available: true },
      { id: "signals", available: true },
      { id: "events", available: false, reason: "Start the runtime or open a retained recording first" },
    ], "Monitor must open on an honest empty response plot, not jump to another tab while connecting");
    assert.strictEqual(
      plots?.("Monitor", { samples: false, recording: true })?.find((plot) => plot.id === "events")?.available,
      true,
      "reopened retained evidence must make its Events tab usable even without a live runtime sample"
    );
    assert.deepStrictEqual(plots?.("Tune", { model: true, candidate: false, mimo: false }), [
      { id: "response", available: false, reason: "Generate or select a controller candidate first" },
      { id: "bode", available: true },
      { id: "root-locus", available: true },
      { id: "nyquist", available: true },
      { id: "nichols", available: false, reason: "Generate or select a controller candidate first" },
      { id: "sensitivity", available: false, reason: "Generate or select a controller candidate first" },
    ]);
    assert.deepStrictEqual(plots?.("Identify", { model: true, candidate: true, mimo: true })?.map((plot) => plot.id), [
      "fit", "residuals", "validation-data", "poles-zeros", "channel-responses", "rga",
    ]);
    const select = (chartContractModule as unknown as {
      selectControlPlot?: (tabs: Array<{ id: string; available: boolean }>, selected?: string) => string | undefined;
    }).selectControlPlot;
    assert.strictEqual(typeof select, "function", "valid direct-plot fallback is not implemented");
    assert.strictEqual(select?.(plots?.("Tune", { model: true, candidate: false }) ?? [], "response"), "bode");
    const view = (chartContractModule as unknown as {
      controlPlotView?: (stage: string, plot: string) => string;
    }).controlPlotView;
    assert.strictEqual(typeof view, "function", "direct plot-to-view routing is not implemented");
    assert.strictEqual(view?.("Test", "experiment"), "response");
    assert.strictEqual(view?.("Test", "limits"), "response");
    assert.strictEqual(view?.("Tune", "response"), "engineering-response");
    assert.strictEqual(view?.("Identify", "residuals"), "residuals");
  });

  test("uses aligned lanes instead of silently overlaying three unit groups", () => {
    const layout = (chartContractModule as unknown as {
      controlSignalLayout?: (signals: Array<{ id: string; unit: string }>) => unknown;
    }).controlSignalLayout;
    assert.strictEqual(typeof layout, "function", "unit-safe signal layout is not implemented");
    assert.deepStrictEqual(layout?.([
      { id: "sp", unit: "°C" }, { id: "pv", unit: "°C" },
      { id: "mv", unit: "%" }, { id: "flow", unit: "L/min" },
    ]), {
      mode: "lanes",
      groups: [
        { unit: "°C", signalIds: ["sp", "pv"], axis: 0 },
        { unit: "%", signalIds: ["mv"], axis: 0 },
        { unit: "L/min", signalIds: ["flow"], axis: 0 },
      ],
    });
  });

  test("accepts only dimensionally complete named MIMO plot evidence", () => {
    const decode = (chartContractModule as unknown as {
      decodeMimoPlotEvidence?: (value: unknown) => unknown;
    }).decodeMimoPlotEvidence;
    assert.strictEqual(typeof decode, "function", "typed MIMO chart evidence is not implemented");
    const evidence = {
      schemaVersion: 1,
      modelRevision: "model-2x2-v1",
      evidenceRevision: "experiment-pair-v1",
      inputs: [{ id: "reflux", name: "Reflux", unit: "pp" }, { id: "steam", name: "Steam", unit: "pp" }],
      outputs: [{ id: "top", name: "Top purity", unit: "pp" }, { id: "bottom", name: "Bottom purity", unit: "pp" }],
      channelResponses: [
        { inputId: "reflux", outputId: "top", points: [[0, 0], [20, 0.8]] },
        { inputId: "steam", outputId: "top", points: [[0, 0], [20, -0.4]] },
        { inputId: "reflux", outputId: "bottom", points: [[0, 0], [20, -0.3]] },
        { inputId: "steam", outputId: "bottom", points: [[0, 0], [20, 0.7]] },
      ],
      relativeGainArray: [[1.27, -0.27], [-0.27, 1.27]],
      singularValues: [{ frequencyRadPerSecond: 0.1, values: [1.2, 0.5] }],
    };
    assert.deepStrictEqual(decode?.(evidence), evidence);
    assert.strictEqual(decode?.({ ...evidence, relativeGainArray: [[1.27, -0.27]] }), undefined);
    assert.strictEqual(decode?.({ ...evidence, channelResponses: evidence.channelResponses.slice(1) }), undefined);
    assert.strictEqual(decode?.({ ...evidence,
      channelResponses: [...evidence.channelResponses, evidence.channelResponses[0]] }), undefined);
  });

  test("keeps graphical edits undoable and pins only compatible designs", () => {
    const contract = chartContractModule as unknown as {
      createDesignHistory?: (candidate: unknown, boundary: string) => unknown;
      commitDesignEdit?: (history: unknown, candidate: unknown, boundary: string) => unknown;
      undoDesignEdit?: (history: unknown, boundary: string) => unknown;
      redoDesignEdit?: (history: unknown, boundary: string) => unknown;
      pinComparableDesign?: (pins: unknown[], candidate: unknown, context: unknown) => unknown[];
    };
    assert.strictEqual(typeof contract.createDesignHistory, "function", "candidate edit history is not implemented");
    const initial = { revision: 1, proportionalGain: 1 };
    const changed = { revision: 2, proportionalGain: 2 };
    const boundary = "loop-a:model-1";
    const committed = contract.commitDesignEdit?.(
      contract.createDesignHistory?.(initial, boundary), changed, boundary
    );
    assert.deepStrictEqual((contract.undoDesignEdit?.(committed, boundary) as { current: unknown }).current, initial);
    assert.deepStrictEqual((contract.redoDesignEdit?.(
      contract.undoDesignEdit?.(committed, boundary), boundary
    ) as { current: unknown }).current, changed);
    assert.throws(() => contract.undoDesignEdit?.(committed, "loop-b:model-1"), /history boundary/i);

    const context = { modelRevision: "model-1", evidenceRevision: "run-1", method: "qualified-v1" };
    const pins = contract.pinComparableDesign?.([], changed, context) ?? [];
    assert.strictEqual(pins.length, 1);
    assert.throws(() => contract.pinComparableDesign?.(pins, initial,
      { ...context, modelRevision: "model-2" }), /incompatible model revision/i);
  });

  test("restores one evidence-bound direct chart session", () => {
    const decode = (chartContractModule as unknown as {
      decodeControlChartSession?: (value: unknown, boundary: string) => unknown;
    }).decodeControlChartSession;
    assert.strictEqual(typeof decode, "function", "chart session restoration is not implemented");
    const session = {
      schemaVersion: 1,
      boundary: "loop-a:model-1",
      activeStage: "Tune",
      selectedPlots: { Monitor: "response", Tune: "bode" },
      selectedSignals: ["sp", "pv", "mv"],
      window: [20, 70],
      expanded: true,
      candidateRevision: 3,
      pinnedCandidateRevisions: [1, 2],
    };
    assert.deepStrictEqual(decode?.(session, session.boundary), session);
    assert.strictEqual(decode?.(session, "loop-b:model-1"), undefined,
      "a session from another loop or model must not be restored as current");
    assert.strictEqual(decode?.({ ...session, selectedSignals: ["sp", "sp"] }, session.boundary), undefined,
      "signal selection order must not contain duplicate channels");
  });

  test("predicts a loop whose gain stays below one and reports its phase margin as unbounded", () => {
    // P with Kp·K = 0.75: |L(jω)| < 1 at every frequency, so there is no gain crossover. The response is well defined.
    const model = { kind: "first_order" as const, gain: 1, timeConstantSeconds: 1, stable: true, normalizedError: 0.01 };
    const parameters: ControlPidParameters = {
      proportionalGain: 0.75, integralGain: 0, derivativeGain: 0.0375,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -20, outputMaximum: 20, rateMinimum: -1e6, rateMaximum: 1e6,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const candidate = candidateFromControlPid(parameters, "Constrained comparison");
    let evidence: ReturnType<typeof buildEngineeringPlotEvidence> | undefined;
    assert.doesNotThrow(() => { evidence = buildEngineeringPlotEvidence(model, candidate, candidate, undefined,
      { cycleTimeSeconds: 0.02, durationSeconds: 4, referenceChange: 1 }); });
    const final = evidence!.timeResponse[evidence!.timeResponse.length - 1].candidatePv;
    assert.ok(Math.abs(final - 0.75 / 1.75) < 1e-3, `settles at Kp·K/(1 + Kp·K): ${final}`);
    assert.strictEqual(evidence!.phaseMarginDegrees, Number.POSITIVE_INFINITY);
    assert.strictEqual(engineeringPlotMessageEvidence(evidence!).phaseMarginDegrees, "unbounded", "infinity survives the message as unbounded");
  });

  test("predicts a slow process over hours at every controller period and keeps at most 5,000 evenly spaced points (A7, A4.5)", () => {
    // A 75 min time constant over a 4 h test at a 100 ms task: 144,000 periods, more than the old 10,000-period cap.
    const model = { kind: "first_order" as const, gain: 0.2, timeConstantSeconds: 4500, stable: true, normalizedError: 0.01 };
    const parameters: ControlPidParameters = {
      proportionalGain: 20, integralGain: 0.01, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -100, outputMaximum: 100, rateMinimum: -1e9, rateMaximum: 1e9,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const candidate = candidateFromControlPid(parameters, "Constrained comparison");
    const evidence = buildEngineeringPlotEvidence(model, candidate, candidate, undefined,
      { cycleTimeSeconds: 0.1, durationSeconds: 14400, referenceChange: 1 });
    const points = evidence.timeResponse;
    assert.ok(points.length <= 5001, `at most 5,000 points after the start: ${points.length}`);
    assert.ok(Math.abs(points[points.length - 1].timeSeconds - 14400) < 1e-6, "the last period is kept");
    const spacing = points[1].timeSeconds - points[0].timeSeconds;
    assert.ok(points.every((point, index) => index === 0 || Math.abs(point.timeSeconds - points[index - 1].timeSeconds - spacing) < 1e-6
      || index === points.length - 1), "the kept periods are evenly spaced");
    assert.ok(Math.abs(points[points.length - 1].candidatePv - 1) < 0.02, `the integral action reaches the target: ${points[points.length - 1].candidatePv}`);
    assert.strictEqual(evidence.uncertainty.points.length === 0 || evidence.uncertainty.points.length === points.length, true,
      "the uncertainty band has one point per kept period");
  });

  test("keeps direct engineering evidence serializable for a compatible saved result", () => {
    const model = { kind: "first_order" as const, gain: 1, timeConstantSeconds: 2, stable: true, normalizedError: 0.01 };
    const parameters: ControlPidParameters = {
      proportionalGain: 0.4, integralGain: 0.1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const current = candidateFromControlPid(parameters, "Imported");
    const candidate = candidateFromControlPid({ ...parameters, proportionalGain: 0.6 }, "Manual");
    const evidence = buildEngineeringPlotEvidence(model, current, candidate);
    const serialized = engineeringPlotMessageEvidence(evidence);
    assert.ok(serialized.timeResponse.length > 0, "saved direct plot evidence must retain the response trace");
    assert.ok(serialized.candidateOpenLoopBode.length > 0, "saved direct plot evidence must retain frequency evidence");
    assert.strictEqual(serialized.evidence, "Model-predicted");
  });

  test("renders one response plot in the current three-page shell without legacy chart catalogues", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /data-studio-three-page/);
    assert.match(template, /data-studio-model-result/);
    assert.match(template, /data-studio-tuning-result/);
    assert.match(template, /data-studio-validation-result/);
    assert.match(template, /data-control-studio-chart/);
    assert.match(template, /data-engineer-apply/);
    assert.match(workflow, /bindThreePageHostState/);
    assert.match(workflow, /renderValidationResults/);
    assert.doesNotMatch(template, /plot-tools|data-control-plot-tabs|data-control-signals|data-design-handle|analysis-grid/);
  });
});

