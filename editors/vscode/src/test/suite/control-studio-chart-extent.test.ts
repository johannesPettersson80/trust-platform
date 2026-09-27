import * as assert from "assert";
import { axis } from "../../controlStudio/webview/chartPrimitives";
import { emptyAxesOption } from "../../controlStudio/webview/plotStates";
import { liveSignalPlot, liveSignals } from "../../controlStudio/webview/liveSignalPlot";
const echarts = require("echarts");

suite("Control Studio populated chart extents", () => {
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
      const labels = chart.getModel().getComponent("yAxis").axis.getViewLabels()
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
  test("keeps unselected diagnostics out of the rendered response legend", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const samples = [0, 1].map(elapsedSeconds => ({ elapsedSeconds, sp: null, pv: 35, mv: 75,
      additional: [{ id: "experiment-authority", channel: "experiment", role: "state", unit: "", value: 0, quality: "good" as const }] }));
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 740, height: 500 });
    try {
      chart.setOption({ animation: false, ...liveSignalPlot(samples, liveSignals(samples, "°C", "%", "open-loop"),
        new Set(["pv", "mv"]), { width: 740, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme) });
      const svg: string = chart.renderToSVGString();
      assert.ok(svg.includes("Measured output") && svg.includes("Applied input"));
      assert.ok(!svg.includes("Experiment authority"), "unselected diagnostics belong in Signals, not the plotted legend");
    } finally { chart.dispose(); }
  });
  test("names paired process signals on the rendered axis without inventing controller state", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const samples = [0, 1].map(elapsedSeconds => ({ elapsedSeconds, sp: null, pv: 35, mv: 75,
      additional: [
        { id: "model", displayName: "Model temperature", channel: "model", role: "state", unit: "degC", value: 35, quality: "good" as const },
        { id: "heat", displayName: "Heater power", channel: "heat", role: "state", unit: "kW", value: 150, quality: "good" as const },
        { id: "loss", displayName: "Loss power", channel: "loss", role: "state", unit: "kW", value: 150, quality: "good" as const },
      ] }));
    for (const [selected, expected] of [
      [["pv", "model", "mv"], "Measured output / Model temperature (°C)"],
      [["heat", "loss"], "Heater power / Loss power (kW)"],
    ] as const) {
      const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 1440, height: 500 });
      try {
        chart.setOption({ animation: false, ...liveSignalPlot(samples, liveSignals(samples, "°C", "%", "open-loop"),
          new Set<string>(selected), { width: 1440, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme) });
        const svg: string = chart.renderToSVGString();
        assert.ok(svg.includes(expected), `the rendered axis must identify its selected pair: ${expected}`);
        assert.ok(!svg.includes("Controller state"), "process model and energy signals are not controller state");
      } finally { chart.dispose(); }
    }
  });
  test("does not duplicate the rounded measured-axis endpoint label while retaining actuator bounds", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const pv = [35, 35, 35, 35, 35, 35, 35.01, 35.02, 35.03, 35.04, 35.05, 35.06, 35.07842112169535];
    const samples = pv.map((value, elapsedSeconds) => ({ elapsedSeconds: elapsedSeconds / 10, sp: null, pv: value,
      mv: elapsedSeconds < 11 ? 85 : 0 }));
    const signals = liveSignals(samples, "°C", "%", "open-loop");
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 1440, height: 500 });
    try {
      chart.setOption({ animation: false, ...liveSignalPlot(samples, signals, new Set(["pv", "mv"]),
        { width: 1440, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme) });
      chart.renderToSVGString();
      const labels = chart.getZr().storage.getDisplayList().filter((item: { type: string; style: { text?: string } }) =>
        item.type === "tspan" && ["35.08", "0", "100"].includes(item.style.text ?? ""));
      const bounds: { text: string; x: number; y: number; width: number; height: number }[] = labels.map((item: { style: { text?: string }; getBoundingRect(): { clone(): { applyTransform(transform: unknown): void; x: number; y: number; width: number; height: number } }; transform: unknown }) => {
        const rect = item.getBoundingRect().clone(); rect.applyTransform(item.transform);
        return { text: item.style.text ?? "", x: rect.x, y: rect.y, width: rect.width, height: rect.height };
      });
      const measuredEndpoint = chart.convertToPixel({ yAxisIndex: 0 }, chart.getModel().getComponent("yAxis", 0).axis.scale.getExtent()[1]) as number;
      const endpointLabels = bounds.filter(label => label.text === "35.08" && label.x < 180 &&
        Math.abs(label.y + label.height / 2 - measuredEndpoint) < 36);
      assert.strictEqual(endpointLabels.length, 1,
        `the padded measured-output endpoint must have one rounded label, got ${endpointLabels.length}`);
      assert.ok(bounds.some(label => label.text === "0" && label.x > 1300), "the actuator minimum label must remain visible");
      assert.ok(bounds.some(label => label.text === "100" && label.x > 1300), "the actuator maximum label must remain visible");
    } finally { chart.dispose(); }
  });
  test("keeps arithmetic roundoff visually constant without flattening a real small response", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    for (const [minimum, maximum, constant] of [[0.29999999999999716, 0.30000000000000426, true], [0.3, 0.300001, false]] as const) {
      const samples = [minimum, maximum].map((value, elapsedSeconds) => ({ elapsedSeconds, sp: null, pv: null, mv: null,
        additional: [{ id: "difference", channel: "reference", role: "state", unit: "degC", value, quality: "good" as const }] }));
      const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 1280, height: 500 });
      try {
        chart.setOption({ animation: false, ...liveSignalPlot(samples, liveSignals(samples, "°C", "%"), new Set(["difference"]),
          { width: 1280, height: 500 }, undefined, "%", theme) });
        chart.renderToSVGString();
        const pixels = Math.abs(chart.convertToPixel({ yAxisIndex: 0 }, maximum) - chart.convertToPixel({ yAxisIndex: 0 }, minimum));
        assert.ok(constant ? pixels < 1 : pixels > 100,
          constant ? "floating-point roundoff must not become a full-height apparent oscillation" : "a real small response must remain inspectable");
        assert.deepStrictEqual(chart.getOption().series[3].data.map((point: number[]) => point[1]), [minimum, maximum],
          "framing must not round or replace the recorded samples");
      } finally { chart.dispose(); }
    }
  });
  test("names an isolated diagnostic axis after its selected signal", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const samples = [0, 1].map(elapsedSeconds => ({ elapsedSeconds, sp: 35, pv: 34, mv: 75,
      additional: [{ id: "error", displayName: "Error", channel: "heater", role: "state", unit: "degC", value: 1, quality: "good" as const }] }));
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 1280, height: 500 });
    try {
      chart.setOption({ animation: false, ...liveSignalPlot(samples, liveSignals(samples, "°C", "%"), new Set(["error"]),
        { width: 1280, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme) });
      assert.ok(chart.renderToSVGString().includes("Error (°C)"),
        "the rendered Error-only axis must name the signal rather than a generic controller-state category");
    } finally { chart.dispose(); }
  });
  test("keeps a live terminal time label from colliding with the preceding tick", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333", background: "#111",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "yellow" };
    const samples = [0, 2.06].map(elapsedSeconds => ({ elapsedSeconds, sp: 35, pv: 35, mv: 75, additional: [{ id: "unavailable", channel: "heater", role: "state", unit: "percentage points", value: 0, quality: "good" as const }] }));
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 640, height: 542 });
    try {
      chart.setOption({ animation: false });
      chart.setOption(liveSignalPlot(samples, liveSignals(samples, "°C", "%"), new Set(["sp", "pv", "mv", "unavailable"]),
        { width: 640, height: 542 }, { minimum: 0, maximum: 100 }, "%", theme));
      chart.renderToSVGString();
      const labels = chart.getZr().storage.getDisplayList().filter((item: { type: string; style: { text?: string } }) =>
        item.type === "tspan" && ["2", "2.06"].includes(item.style.text ?? ""));
      assert.ok(labels.length > 0, "the final time region must remain labeled");
      if (labels.length === 2) {
        const bounds = labels.map((item: { getBoundingRect(): { clone(): { applyTransform(transform: unknown): void; x: number; width: number } }; transform: unknown }) => {
          const rect = item.getBoundingRect().clone(); rect.applyTransform(item.transform); return rect;
        }).sort((a: { x: number }, b: { x: number }) => a.x - b.x);
        assert.ok(bounds[0].x + bounds[0].width + 2 <= bounds[1].x,
          "rendered time labels 2 and2.06 must not overlap at the current observation boundary");
      }
      assert.ok(chart.getModel().getComponent("xAxis").axis.scale.getExtent()[1] >= 2.06,
        "deconflicting labels must preserve the terminal recorded sample");
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
  test("replaces empty-state bounds with the entire received response", () => {
    const theme = { foreground: "#eee", muted: "#aaa", border: "#333" } as any;
    const chart = echarts.init(null, null, { renderer: "svg", ssr: true, width: 640, height: 400 });
    try {
      chart.setOption(emptyAxesOption("Time (s)", "PV", theme));
      chart.setOption({ xAxis: axis("Time (s)", "value", theme, [0, 1, 2]),
        yAxis: axis("PV", "value", theme, [0, 2, 3.5]),
        series: [{ type: "line", data: [[0, 0], [1, 2], [2, 3.5]] }] });
      assert.ok(chart.renderToSVGString().includes("<svg"));
      assert.ok(chart.getModel().getComponent("xAxis").axis.scale.getExtent()[1] >= 2,
        "The final received sample at 2 s must be visible, not clipped at the empty-state 1 s bound");
      assert.ok(chart.getModel().getComponent("yAxis").axis.scale.getExtent()[1] >= 3.5,
        "The response amplitude must not be clipped by empty-state bounds");
    } finally { chart.dispose(); }
  });
});
