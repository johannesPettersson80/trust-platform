import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as chartContractModule from "../../controlStudio/chartContract";
import type { PlotSignal } from "../../controlStudio/generated/OpenResult";
import { responseChartSeries, responseChartTooltip } from "../../controlStudio/webview/threePageResponseSeries";
import { rootLocusChartSeries } from "../../controlStudio/webview/rootLocusChart";
import { sensitivitySeries } from "../../controlStudio/webview/sensitivityChart";
import { nyquistGeometry, nyquistSeries } from "../../controlStudio/webview/nyquistChart";
import { initialStateResponseRows, initialStateResponseSeries } from "../../controlStudio/webview/initialStateResponseChart";
import { poleMapGeometry } from "../../controlStudio/webview/complexPlaneGeometry";

suite("Control Studio chart contract", () => {

  const threePageTheme = { foreground: "black", muted: "gray", border: "gray", background: "white",
    sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "gold" };
  // The signals the engine lists for the plots (`liveSides.plotSignals`): a setpoint, a measurement, an output and nine states.
  const threePageSignals = (): Pick<PlotSignal, "id" | "kind">[] => [
    { id: "sp", kind: "setpoint" }, { id: "pv", kind: "measurement" }, { id: "mv", kind: "appliedOutput" },
    ...Array.from({ length: 9 }, (_, index) => ({ id: `state-${index}`, kind: "additional" as const }))];
  const threePagePlot = (signals: Pick<PlotSignal, "id" | "kind">[], ids: string[], prediction = false) => responseChartSeries(threePageTheme, signals,
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
    const signals = threePageSignals();
    const ids = signals.map(signal => signal.id);
    const all = threePagePlot(signals, ids);
    assert.deepStrictEqual(all.slice(0, 3).map(threePageIdentity), ["green/dashed", "blue/solid", "red/solid"]);
    assert.deepStrictEqual(threePagePlot(signals, ["pv", "state-4"]).map(threePageIdentity), [all[1], all[7]].map(threePageIdentity),
      "signal selection must not reassign the remaining trace identities");
    assert.strictEqual((threePagePlot(signals, ["pv"], true)[0].lineStyle as { color?: string }).color, "orange",
      "a model prediction keeps its prediction stroke");
  });

  test("the three-page response plot never draws two displayed signals alike", () => {
    const signals = threePageSignals();
    const ids = signals.map(signal => signal.id);
    const all = threePagePlot(signals, ids);
    assert.strictEqual(new Set(all.map(threePageIdentity)).size, ids.length,
      `each displayed trace needs its own colour/style pair: ${all.map(threePageIdentity).join(", ")}`);
  });

  test("the output the controller requested is drawn in an auxiliary stroke, as the engine's kind says", () => {
    // A loop with an actuator lists the requested output (`requestedMv`) beside the applied one (`mv`). The engine gives it a kind of its own and the plot
    // groups the kinds once: a page that coloured by a list of three fixed identifiers (sp, pv, mv) and named by another list of four would treat the same
    // signal two ways.
    const signals: Pick<PlotSignal, "id" | "kind">[] = [...threePageSignals().slice(0, 3), { id: "requestedMv", kind: "requestedOutput" }, { id: "state-0", kind: "additional" }];
    const [applied, requested, state] = threePagePlot(signals, ["mv", "requestedMv", "state-0"]);
    assert.strictEqual(threePageIdentity(applied), "red/solid", "the applied output keeps its own colour");
    assert.ok(!["green", "blue", "red"].includes((requested.lineStyle as { color?: string }).color ?? ""), "the requested output takes an auxiliary colour");
    assert.notStrictEqual(threePageIdentity(requested), threePageIdentity(state), "and a stroke of its own among the auxiliary signals");
    assert.deepStrictEqual([applied, requested].map(series => (series as { step?: unknown }).step), ["end", undefined], "only the applied output is held between samples");
  });

  test("the three-page response plot draws the applied output as held between samples", () => {
    const [pv, mv] = threePagePlot(threePageSignals(), ["pv", "mv"]);
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

  test("keeps candidate settings in the Tune result area without legacy margin cards", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    assert.match(template, /data-studio-tuning-result/);
    assert.doesNotMatch(template, /data-design-gain-margin|data-design-phase-margin/);
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
    // The Validate answer reaches its receiver through the host state's engine messages (threePageHostState.ts, engineStages.ts), which
    // also relays the offers the answer states to the Apply area.
    assert.match(fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageHostState.ts"), "utf8"), /receiveEngineMessage/);
    const stages = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "engineStages.ts"), "utf8");
    assert.match(stages, /\["validate", validateStage\]/);
    assert.match(stages, /validateReceiver\.result\?\.\(root, result, ports\)/);
    assert.doesNotMatch(template, /plot-tools|data-control-plot-tabs|data-control-signals|data-design-handle|analysis-grid/);
  });
});
