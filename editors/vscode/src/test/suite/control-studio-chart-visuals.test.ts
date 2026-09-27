import * as assert from "assert";

import { axisTickFormatter, frequencyAxisTickFormatter, linearAxisFractionDigits } from "../../controlStudio/webview/axisFormat";
import {
  magnitudeCrossoverMarkers,
  magnitudeReferenceLines,
  nicholsCrossoverMarkers,
  nicholsReferenceLines,
  phaseCrossoverMarkers,
  phaseReferenceLines,
  type MarginMarkerInput,
} from "../../controlStudio/webview/marginMarkers";
import { uncertaintyBandDescription, uncertaintyBandSeries } from "../../controlStudio/webview/uncertaintyBand";
import { emptyAxesOption, plotStateMessage, plotStateOverlay } from "../../controlStudio/webview/plotStates";
import { valueStripHistoricalState } from "../../controlStudio/webview/valueStripState";
import { readableSignalName } from "../../controlStudio/signalNaming";
import { displayEngineeringUnit } from "../../controlStudio/unitDisplay";
import { automaticFractionDigits, CONTROL_CHART_SIGNALS, liveResponseAxisRange } from "../../controlStudio/chartContract";
import { buildEngineeringPlotEvidence } from "../../controlStudio/engineeringPlots";
import { candidateFromControlPid, type ControlPidParameters } from "../../controlStudio/controllerCandidate";
import type { IdentifiedModel } from "../../controlStudio/identification";

// ECharts calls a value-axis formatter as (value, index); this cast reproduces that call
// shape against a formatter typed with just (value) so the test fails the same way a real
// axis would if the index ever leaked back in as a fraction-digit count (finding V-01).
function callAsEcharts(formatter: (value: unknown) => string, value: unknown, index: number): string {
  return (formatter as unknown as (value: unknown, index: number) => string)(value, index);
}

suite("Control Studio strict-review visual fixes", () => {
  test("keeps signal colors attached to PV and SP across response views", () => {
    const { JSDOM } = require("jsdom");
    const { chartTheme } = require("../../controlStudio/webview/chartPrimitives");
    const dom = new JSDOM('<html style="--vscode-charts-blue:#005fb8;--vscode-charts-green:#228822;--vscode-charts-red:#d00000"></html>');
    const globals = global as unknown as { document?: Document; getComputedStyle?: typeof getComputedStyle };
    const original = { document: globals.document, getComputedStyle: globals.getComputedStyle };
    globals.document = dom.window.document; globals.getComputedStyle = dom.window.getComputedStyle.bind(dom.window);
    try {
      const theme = chartTheme();
      const color = (value: string) => {
        const element = dom.window.document.createElement("span");
        element.style.color = value;
        return element.style.color;
      };
      assert.strictEqual(color(theme.pv), color("#005fb8"), "measured PV retains the live response blue in every view");
      assert.strictEqual(color(theme.sp), color("#228822"), "setpoint retains the live response green in every view");
      assert.strictEqual(color(theme.mv), color("#d00000"));
    } finally { Object.assign(globals, original); dom.window.close(); }
  });
  test("V-01: axis formatter ignores ECharts' tick index instead of using it as fraction digits", () => {
    const formatter = axisTickFormatter();
    // Reproduces the reported bug: tick 0 -> index 0, tick 1 -> index 1, tick 7 -> index 7.
    assert.strictEqual(callAsEcharts(formatter, 30, 0), "30");
    assert.strictEqual(callAsEcharts(formatter, 30, 1), "30", "the same value must format identically regardless of tick index");
    assert.strictEqual(callAsEcharts(formatter, 31.641848, 7), "31.64",
      "a tick index of 7 must not become 7 fraction digits");

    const capped = axisTickFormatter(3);
    assert.strictEqual(callAsEcharts(capped, 31.641848, 9), "31.642",
      "an explicit fraction-digit count must also ignore the tick index");
  });

  test("V-01: linear axis fraction digits derive from the axis span, not a degenerate per-tick guess", () => {
    assert.strictEqual(linearAxisFractionDigits([28, 31.64, 30]), 2, "magnitude >= 1 uses 2 digits");
    assert.strictEqual(linearAxisFractionDigits([0, 0.05, 0.02]), 4, "magnitude < 1 uses 4 digits");
    assert.strictEqual(linearAxisFractionDigits([5, 5, 5]), undefined,
      "a flat span carries no span signal and must fall back to automatic formatting");
    assert.strictEqual(linearAxisFractionDigits([null, undefined]), undefined);
    assert.strictEqual(linearAxisFractionDigits([]), undefined);
    assert.strictEqual(automaticFractionDigits(31.64), 2, "the axis-span policy reuses the single automatic-digits policy");
  });

  test("V-05: frequency ticks stay in plain decimal notation across the plotted decades", () => {
    const formatter = frequencyAxisTickFormatter();
    for (const [value, expected] of [[0.001, "0.001"], [0.1, "0.1"], [1, "1"], [10, "10"], [100, "100"]] as const) {
      assert.strictEqual(callAsEcharts(formatter, value, 3), expected,
        `frequency tick ${value} must not switch to exponential notation next to plain-notation neighbours`);
    }
  });

  test("V-02/V-07: one unit-display helper maps configuration unit spellings to what a student reads", () => {
    assert.strictEqual(displayEngineeringUnit("degC"), "°C");
    assert.strictEqual(displayEngineeringUnit("pp"), "percentage points");
    assert.strictEqual(displayEngineeringUnit("%/degC"), "%/°C", "a composite unit must still map its degC token");
    assert.strictEqual(displayEngineeringUnit("%/(degC*s)"), "%/(°C*s)");
    assert.strictEqual(displayEngineeringUnit("pp/pp"), "percentage points/percentage points");
    assert.strictEqual(displayEngineeringUnit("level pp"), "level percentage points");
    assert.strictEqual(displayEngineeringUnit("%"), "%", "an unrecognized unit passes through unchanged");
    assert.strictEqual(displayEngineeringUnit("  degC  "), "°C", "surrounding whitespace is trimmed");
  });

  test("V-08: the live response axis keeps 5-10 percent headroom instead of autoscaling tight to the data", () => {
    const range = liveResponseAxisRange([28, 30, 31, 29]);
    assert.ok(range);
    assert.ok(range!.minimum < 28 && range!.maximum > 31, "PV/SP must not sit exactly on the frame edge");
    const span = 31 - 28;
    assert.ok((28 - range!.minimum) >= span * 0.05 && (28 - range!.minimum) <= span * 0.12);
    assert.deepStrictEqual(liveResponseAxisRange([0, 0]), { minimum: -1, maximum: 1 }, "An idle zero signal must not imply micro-unit measurement precision");
    assert.strictEqual(liveResponseAxisRange([]), undefined, "no samples yet must not force a degenerate range");
    assert.strictEqual(liveResponseAxisRange([null, undefined]), undefined);
  });

  test("V-08: SP is dashed so it stays visible drawn on top of a coinciding solid PV trace", () => {
    const sp = CONTROL_CHART_SIGNALS.find((signal) => signal.id === "sp");
    const pv = CONTROL_CHART_SIGNALS.find((signal) => signal.id === "pv");
    assert.strictEqual(sp?.line, "dashed");
    assert.strictEqual(pv?.line, "solid");
  });

  test("W-09: the manipulated-variable tile name comes from its own bound symbol", () => {
    assert.strictEqual(readableSignalName("P1.AppliedHeaterPower"), "Applied heater power");
    assert.strictEqual(readableSignalName("P1.RequestedHeaterPower"), "Requested heater power");
    assert.strictEqual(readableSignalName("P1.ManualDemand"), "Manual demand");
    assert.strictEqual(readableSignalName("P1.ManualHeaterPower"), "Manual heater power");
    assert.strictEqual(readableSignalName("MV"), "MV", "a short role code is kept as-is instead of forced to sentence case");
    assert.strictEqual(readableSignalName("P1.MV"), "MV");
    assert.strictEqual(readableSignalName(""), "");
  });

  test("V-05: margin markers place 0 dB / -180 degree references and GM/PM points from the resolved crossovers", () => {
    const input: MarginMarkerInput = {
      gainMarginDb: 12.5,
      phaseMarginDegrees: 44.2,
      gainCrossover: { frequencyRadPerSecond: 1.5, magnitudeDb: 0, phaseDegrees: -135.8 },
      phaseCrossover: { frequencyRadPerSecond: 4.2, magnitudeDb: -12.5, phaseDegrees: -180 },
    };
    assert.deepStrictEqual(magnitudeReferenceLines(input), [
      { name: "0 dB", yAxis: 0 },
      { name: "Phase crossover", xAxis: 4.2 },
    ]);
    assert.deepStrictEqual(magnitudeCrossoverMarkers(input), [
      { name: "Gain margin", coord: [4.2, -12.5], value: "GM 12.5 dB" },
    ]);
    assert.deepStrictEqual(phaseReferenceLines(input), [
      { name: "−180°", yAxis: -180 },
      { name: "Gain crossover", xAxis: 1.5 },
    ]);
    assert.deepStrictEqual(phaseCrossoverMarkers(input), [
      { name: "Phase margin", coord: [1.5, -135.8], value: "PM 44.2°" },
    ]);
    assert.deepStrictEqual(nicholsReferenceLines(), [{ name: "0 dB", yAxis: 0 }, { name: "−180°", xAxis: -180 }]);
    assert.deepStrictEqual(nicholsCrossoverMarkers(input), [
      { name: "Phase margin", coord: [-135.8, 0], value: "PM 44.2°" },
      { name: "Gain margin", coord: [-180, -12.5], value: "GM 12.5 dB" },
    ]);
  });

  test("V-05: an unbounded gain margin draws the 0 dB reference but no GM marker", () => {
    const input: MarginMarkerInput = {
      gainMarginDb: Number.POSITIVE_INFINITY,
      phaseMarginDegrees: 61.4,
      gainCrossover: { frequencyRadPerSecond: 0.8, magnitudeDb: 0, phaseDegrees: -118.6 },
      phaseCrossover: undefined,
    };
    assert.deepStrictEqual(magnitudeReferenceLines(input), [{ name: "0 dB", yAxis: 0 }]);
    assert.deepStrictEqual(magnitudeCrossoverMarkers(input), []);
    assert.strictEqual(nicholsCrossoverMarkers(input).length, 1, "only the resolved phase-margin point is plotted");
  });

  test("V-05/engineeringPlots: the candidate evidence exposes the exact crossover points the markers need", () => {
    const model: IdentifiedModel = { kind: "first_order", gain: 1.4, timeConstantSeconds: 4, deadTimeSeconds: 0.6, stable: true, normalizedError: 0.05 };
    const base: ControlPidParameters = {
      proportionalGain: 0.4, integralGain: 0.1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const evidence = buildEngineeringPlotEvidence(model,
      candidateFromControlPid(base, "Imported"), candidateFromControlPid(base, "Imported"));
    assert.ok(Number.isFinite(evidence.gainCrossover.frequencyRadPerSecond));
    assert.ok(Math.abs(evidence.gainCrossover.magnitudeDb) < 1e-6, "the gain crossover is defined at 0 dB");
    assert.ok(evidence.phaseCrossover === undefined || Number.isFinite(evidence.phaseCrossover.frequencyRadPerSecond));
  });

  test("V-17: the uncertainty band is labelled upper/lower/range and never colour-only", () => {
    const points = [{ timeSeconds: 0, lower: 0.9, upper: 1.1 }, { timeSeconds: 1, lower: 0.85, upper: 1.2 }];
    const series = uncertaintyBandSeries("sample-resolution envelope", points, "#888888");
    assert.strictEqual(series.length, 3, "lower bound, filled range, and upper bound are three distinct labelled series");
    assert.deepStrictEqual(series.map((entry) => entry.name), [
      "Uncertainty lower bound", "Uncertainty range · sample-resolution envelope", "Uncertainty upper bound",
    ]);
    assert.ok(series[1].areaStyle && series[1].areaStyle.decal, "the range fill must carry a hatch pattern, not colour alone");
    assert.deepStrictEqual(uncertaintyBandSeries("any", [], "#888888"), [], "no evidence draws no band");
    assert.strictEqual(uncertaintyBandDescription("sample-resolution envelope", 2),
      "Uncertainty band: sample-resolution envelope, shaded lower-to-upper range across 2 predicted points.");
    assert.strictEqual(uncertaintyBandDescription("sample-resolution envelope", 0), "Uncertainty: Not evaluated.",
      "missing uncertainty must read as an explicit sentence, never a silently absent band");
  });

  test("V-17/engineeringPlots: the uncertainty method reaches the plot as words, never a raw enum", () => {
    const model: IdentifiedModel = {
      kind: "first_order", gain: 1.4, timeConstantSeconds: 4, stable: true, normalizedError: 0.05,
      uncertainty: { method: "sample-resolution-envelope", gain: { minimum: 1.2, maximum: 1.6 } },
    };
    const base: ControlPidParameters = {
      proportionalGain: 0.4, integralGain: 0.1, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    };
    const evidence = buildEngineeringPlotEvidence(model,
      candidateFromControlPid(base, "Imported"), candidateFromControlPid(base, "Imported"));
    assert.strictEqual(evidence.uncertainty.method, "sample-resolution envelope from the estimation record");
    assert.doesNotMatch(evidence.uncertainty.method, /^[a-z0-9]+(-[a-z0-9]+)+$/,
      "the method must not reach the plot as a dash-case identifier");
  });

  test("V-10: every empty or failed plot state carries one sentence and keeps its axes", () => {
    assert.strictEqual(plotStateMessage("Identify and accept a dynamic model first.", "fallback"),
      "Identify and accept a dynamic model first.");
    assert.strictEqual(plotStateMessage(undefined, "The candidate engineering analysis failed."),
      "The candidate engineering analysis failed.");
    assert.strictEqual(plotStateMessage("   ", "fallback"), "fallback");
    const theme = { foreground: "#fff", muted: "#999" };
    const hidden = plotStateOverlay("id", undefined, theme);
    assert.strictEqual((hidden as { invisible: boolean }).invisible, true);
    const shown = plotStateOverlay("id", "Record a bounded test first.", theme);
    assert.strictEqual((shown as { invisible: boolean }).invisible, false);
    assert.strictEqual((shown as { style: { text: string } }).style.text, "Record a bounded test first.");
    const axes = emptyAxesOption("Time (s)", "PV", theme) as { series: unknown[]; xAxis: { type: string }; yAxis: { type: string } };
    assert.strictEqual(axes.xAxis.type, "value");
    assert.strictEqual(axes.yAxis.type, "value");
    assert.deepStrictEqual(axes.series, [], "an empty state has real axes and zero fabricated curves");
  });

  test("W-16 follow-up: the value strip badges and dims exactly while showing a saved result", () => {
    assert.strictEqual(valueStripHistoricalState(false, "2 September 2026, 10:00"), undefined);
    assert.deepStrictEqual((valueStripHistoricalState as any)(true, "2 September 2026, 10:00", "draft"),
      { badgeText: "Working draft", dimmed: true }, "an editable draft cannot claim to be a saved history revision");
    assert.deepStrictEqual(valueStripHistoricalState(true, "2 September 2026, 10:00"),
      { badgeText: "Saved result · 2 September 2026, 10:00", dimmed: true });
    assert.deepStrictEqual(valueStripHistoricalState(true, undefined), { badgeText: "Saved result", dimmed: true },
      "a saved result must still badge even before its timestamp is available");
  });
});
