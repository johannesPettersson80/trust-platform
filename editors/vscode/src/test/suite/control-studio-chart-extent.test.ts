import * as assert from "assert";
import { axis, formatPhaseDegrees, phaseAxis, phaseAxisRange } from "../../controlStudio/webview/chartPrimitives";
import { responseRowAxis, responseRowGrids } from "../../controlStudio/webview/responseRows";
const echarts = require("echarts");

suite("Control Studio populated chart extents", () => {
  test("keeps each response row's axis name within its row", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const names = ["P1.TemperatureC (°C)", "P1.AppliedPowerPct (%)", "P1.ObservedKp (percentage-point/°C)", "P1.ObservedKi (percentage-point/(°C·s))"];
    const height = 720, pitch = height / names.length;
    const grids = responseRowGrids(names.length, height);
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 1200, height });
    try {
      chart.setOption({ animation: false, grid: grids,
        xAxis: grids.map((_grid, index) => ({ type: "value", gridIndex: index })),
        yAxis: names.map((name, index) => responseRowAxis(name, theme as any, [0, 1], index, true, pitch)),
        series: names.map((_name, index) => ({ type: "line", xAxisIndex: index, yAxisIndex: index, data: [[0, 0], [1, 1]] })) });
      const texts = [...chart.renderToSVGString().matchAll(/<text[^>]*>([^<]*)<\/text>/g)].map(match => match[1]);
      const nameLines = texts.filter(text => /^P1\.|^\(/.test(text));
      assert.ok(nameLines.length >= names.length, `every row is named: ${nameLines.join(" | ")}`);
      for (const line of nameLines) {
        // truncateText returns the text unchanged exactly when it fits the width.
        assert.strictEqual(echarts.format.truncateText(line, pitch - 16, "12px sans-serif", "…"), line,
          `The rotated name "${line}" is longer than its ${pitch} px row and runs into the next row's name`);
      }
    } finally { chart.dispose(); }
  });
  test("renders distinct temperature ticks for a small noise span around a nonzero operating point", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const values = [34.986, 35.002];
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 800, height: 400 });
    try {
      chart.setOption({ animation: false, xAxis: { type: "value" },
        yAxis: axis("Temperature (°C)", "value", theme, values, { fitToData: true }),
        series: [{ type: "line", data: values.map((value, time) => [time, value]) }] });
      const rendered = chart.renderToSVGString();
      // The first and last view labels are the padded ends of a fitted axis, which are deliberately not
      // drawn; the regular ticks between them carry the scale.
      const labels = chart.getModel().getComponent("yAxis").axis.getViewLabels().slice(1, -1)
        .map((label: { formattedLabel: string }) => label.formattedLabel);
      assert.ok(labels.length >= 3);
      assert.strictEqual(new Set(labels).size, labels.length,
        `Distinct temperature ticks must not all round to35°C: ${labels.join(", ")}`);
      assert.ok(labels.every((label: string) => rendered.includes(label)));
    } finally { chart.dispose(); }
  });
  test("renders constant response roundoff as a stationary trace without altering samples", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    for (const values of [[0.29999999999999716, 0.30000000000000426], [0, 0]]) {
      const points = values.map((value, time) => [time, value]);
      const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 740, height: 500 });
      try {
        chart.setOption({ animation: false, xAxis: { type: "value" },
          yAxis: axis("Sensor difference (°C)", "value", theme, values, { fitToData: true }),
          series: [{ type: "line", data: points }] });
        chart.renderToSVGString();
        const [minimum, maximum] = chart.getModel().getComponent("yAxis").axis.scale.getExtent();
        assert.ok(maximum - minimum >= 0.001,
          `A constant engineering signal needs a readable range; got ${minimum} to ${maximum}`);
        const pixels = values.map(value => chart.convertToPixel({ yAxisIndex: 0 }, value));
        assert.ok(Math.abs(pixels[1] - pixels[0]) < 1, "Numerical roundoff must not look like process oscillation");
        assert.deepStrictEqual(chart.getOption().series[0].data, points, "Display framing must preserve exact recorded values");
      } finally { chart.dispose(); }
    }
  });
  test("labels a fitted axis at its regular ticks, not at the padded data extent", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const values = [0, 1.46];
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 800, height: 400 });
    try {
      chart.setOption({ animation: false, xAxis: { type: "value" },
        yAxis: axis("P1.AngleRad (rad)", "value", theme, values, { fitToData: true }),
        series: [{ type: "line", data: values.map((value, time) => [time, value]) }] });
      const rendered: string = chart.renderToSVGString();
      const [minimum, maximum] = chart.getModel().getComponent("yAxis").axis.scale.getExtent();
      assert.ok(minimum < 0 && maximum > 1.46, "the fitted axis keeps its padding around the data");
      for (const extent of ["-0.12", "1.58"]) {
        assert.ok(!rendered.includes(`>${extent}<`), `the padded extent ${extent} must not crowd the regular ticks`);
      }
      for (const tick of ["0", "0.3", "1.5"]) assert.ok(rendered.includes(`>${tick}<`), `regular tick ${tick} stays labelled`);
    } finally { chart.dispose(); }
  });
  test("frames an operating-point response without flattening it against zero", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333" } as any;
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 640, height: 400 });
    try {
      chart.setOption({ xAxis: axis("Time (s)", "value", theme, [0, 1, 2]),
        yAxis: (axis as any)("PV (°C)", "value", theme, [20, 20.5, 21], { fitToData: true }),
        series: [{ type: "line", data: [[0, 20], [1, 20.5], [2, 21]] }] });
      chart.renderToSVGString();
      const [minimum, maximum] = chart.getModel().getComponent("yAxis").axis.scale.getExtent();
      assert.ok(minimum > 19 && minimum <= 20 && maximum >= 21 && maximum < 22,
        `the 1°C response must be readable at its20°C operating point: ${minimum}..${maximum}`);
    } finally { chart.dispose(); }
  });
  test("a phase axis spans whole 45-degree steps around its data, so noise around a zero phase is not scaled up", () => {
    // A pure gain's phase is zero or floating-point noise around zero (2.5e-16 degrees in the screenshot).
    assert.deepStrictEqual(phaseAxisRange([-2.5e-16, 1.9e-16]), { minimum: -45, maximum: 45 });
    assert.deepStrictEqual(phaseAxisRange([0, 2.5e-16]), { minimum: 0, maximum: 45 });
    assert.deepStrictEqual(phaseAxisRange([0, 0]), { minimum: 0, maximum: 45 });
    assert.deepStrictEqual(phaseAxisRange([]), { minimum: 0, maximum: 45 });
    // The data's own range, widened outward to multiples of 45 degrees.
    assert.deepStrictEqual(phaseAxisRange([-100, 0]), { minimum: -135, maximum: 0 });
    assert.deepStrictEqual(phaseAxisRange([-100, -50, null, Number.NaN]), { minimum: -135, maximum: -45 });
    assert.deepStrictEqual(phaseAxisRange([-45, -45]), { minimum: -45, maximum: 0 });
    assert.deepStrictEqual(phaseAxisRange([50, 60]), { minimum: 45, maximum: 90 });
  });
  test("a phase axis is drawn with a tick every 45 degrees over its span", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const phases = [-100, -60, 0];
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 640, height: 400 });
    try {
      chart.setOption({ animation: false, xAxis: { type: "value" },
        yAxis: phaseAxis("Phase (°)", theme, phases),
        series: [{ type: "line", data: phases.map((value, time) => [time, value]) }] });
      const rendered: string = chart.renderToSVGString();
      const [minimum, maximum] = chart.getModel().getComponent("yAxis").axis.scale.getExtent();
      assert.deepStrictEqual([minimum, maximum], [-135, 0]);
      for (const tick of ["-135", "-90", "-45", "0"]) assert.ok(rendered.includes(`>${tick}<`), `phase tick ${tick} is labelled`);
    } finally { chart.dispose(); }
  });
  test("a phase axis that spans several turns doubles its tick until it has at most eight steps", () => {
    // The engine plots an unwrapped phase (numerical annex N4.3): e^(−s)/(s + 1) falls from −1.15° at 0.01 rad/s to −657.25°
    // at 10 rad/s. A 45° tick needs 15 steps (−675 to 0) and a 90° tick 8 (−720 to 0), so the axis takes 90°.
    const delayed = [-1.15, -84.29, -657.25];
    assert.deepStrictEqual(phaseAxisRange(delayed), { minimum: -720, maximum: 0 });
    // A reverse-acting delayed loop from 175° down to −900°: 45° and 90° need 24 and 12 steps, 180° needs 6 (−900 to 180).
    assert.deepStrictEqual(phaseAxisRange([175, -200, -900]), { minimum: -900, maximum: 180 });
    // Eight 45° steps are still drawn at 45°: −350° to 0° spans −360 to 0.
    assert.deepStrictEqual(phaseAxisRange([-350, 0]), { minimum: -360, maximum: 0 });
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    assert.strictEqual(phaseAxis("Phase (°)", theme, [-350, 0]).interval, 45);
    assert.strictEqual(phaseAxis("Phase (°)", theme, [175, -200, -900]).interval, 180);
    const yAxis = phaseAxis("Phase (°)", theme, delayed);
    assert.strictEqual(yAxis.interval, 90);
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 640, height: 400 });
    try {
      chart.setOption({ animation: false, xAxis: { type: "value" }, yAxis,
        series: [{ type: "line", data: delayed.map((value, index) => [index, value]) }] });
      const rendered: string = chart.renderToSVGString();
      const [minimum, maximum] = chart.getModel().getComponent("yAxis").axis.scale.getExtent();
      assert.deepStrictEqual([minimum, maximum], [-720, 0]);
      for (const tick of ["-720", "-630", "-360", "-90", "0"]) assert.ok(rendered.includes(`>${tick}<`), `phase tick ${tick} is labelled`);
      assert.ok(!rendered.includes(">-675<") && !rendered.includes(">-45<"), "no 45° tick between the 90° ticks");
    } finally { chart.dispose(); }
  });
  test("a phase in a result table is shown to 0.01 degree, and a value that rounds to zero shows 0", () => {
    assert.strictEqual(formatPhaseDegrees(1.9e-16), "0");
    assert.strictEqual(formatPhaseDegrees(-0.004), "0");
    assert.strictEqual(formatPhaseDegrees(-0), "0");
    assert.strictEqual(formatPhaseDegrees(-12.5), "-12.50");
    assert.strictEqual(formatPhaseDegrees(45), "45.00");
    assert.strictEqual(formatPhaseDegrees(Number.NaN), "—");
  });
});
