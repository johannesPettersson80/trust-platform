import * as assert from "assert";
import { axis } from "../../controlStudio/webview/chartPrimitives";
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
});
