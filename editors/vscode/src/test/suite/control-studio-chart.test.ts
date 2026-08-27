import * as assert from "assert";

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

suite("Control Studio chart contract", () => {
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
  });

  test("labels live plots with real units and marks only saturation transitions", () => {
    const contract = chartContractModule as unknown as {
      controlChartAxisLabels?: (processUnit: string, outputUnit: string) => unknown;
      controlChartSaturationEvent?: (
        previous: "minimum" | "maximum" | "none" | undefined,
        current: "minimum" | "maximum" | "none"
      ) => unknown;
    };
    assert.strictEqual(typeof contract.controlChartAxisLabels, "function",
      "live chart unit labels are not implemented");
    assert.deepStrictEqual(contract.controlChartAxisLabels?.("°C", "%"), {
      process: "SP / PV (°C)", output: "MV (%)",
      description: "SP and PV in °C; MV in %",
    });
    assert.strictEqual(typeof contract.controlChartSaturationEvent, "function",
      "saturation transition projection is not implemented");
    assert.deepStrictEqual(contract.controlChartSaturationEvent?.("none", "maximum"), {
      kind: "limit", label: "Output entered maximum saturation",
    });
    assert.strictEqual(contract.controlChartSaturationEvent?.("maximum", "maximum"), undefined,
      "a saturated scan must not create another plot marker");
    assert.deepStrictEqual(contract.controlChartSaturationEvent?.("maximum", "none"), {
      kind: "limit", label: "Output left maximum saturation",
    });
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
      "solid",
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
});
