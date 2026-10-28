import * as assert from "assert";

import { axisTickFormatter, frequencyAxisTickFormatter, linearAxisFractionDigits } from "../../controlStudio/webview/axisFormat";
import { displayEngineeringUnit } from "../../controlStudio/unitDisplay";
import { automaticFractionDigits, liveResponseAxisRange } from "../../controlStudio/chartContract";

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
    assert.strictEqual(displayEngineeringUnit("%/(degC*s)"), "%/(°C·s)", "a product of units reads with a multiplication dot");
    assert.strictEqual(displayEngineeringUnit("V*s/m"), "V·s/m");
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
});
