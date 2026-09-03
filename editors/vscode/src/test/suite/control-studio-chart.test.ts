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

// panel.ts's markup and styles were split out into panelTemplate.ts and panelStyles.ts;
// every test below that inspects the shipped document or CSS reads all of them together
// so it keeps validating the same rendered surface regardless of which file currently
// owns a given fragment.
function readControlStudioPanelSource(extensionRoot: string): string {
  return ["panel.ts", "panelTemplate.ts", "panelStyles.ts", "stageAvailability.ts"]
    .map((file) => fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", file), "utf8"))
    .join("\n");
}

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
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    assert.match(panel, /\.plot-view>\.analysis-plot\{position:absolute;inset:0\}/,
      "single-panel direct analysis plots must fill the dominant plot region");
    assert.doesNotMatch(panel, /html,body(?:,\.studio)?\{[^}]*overflow-x:clip/,
      "document clipping must not break sticky workflow navigation");
    assert.doesNotMatch(panel, /@media\(max-width:600px\)\{html\{[^}]*overflow-x:clip/,
      "the root viewport must remain available to the studio scroll container");
    assert.match(panel, /@media\(max-width:600px\)\{body\{height:100vh;min-height:0;overflow:hidden\}\.studio\{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden\}/,
      "the narrow studio must own scrolling so its workflow navigation can remain sticky");
    assert.match(panel, /@media\(max-width:600px\)\{\.workspace\{overflow-x:clip\}\}/,
      "workflow content must not widen the page at 200 percent zoom");
    assert.match(panel, /@media\(max-width:600px\)\{\.chart-table\{width:100%;min-width:0;contain:inline-size paint\}\}/,
      "wide engineering tables must scroll inside their own container at 200 percent zoom");
  });

  test("keeps direct charts readable and their states visible in constrained layouts", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");

    assert.match(panel, /@media\(max-width:720px\)\{[\s\S]*?\.studio\{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden\}/,
      "split and narrow layouts need a scroll owner instead of compressing the chart row");
    assert.match(panel, /@media\(max-width:720px\)\{[\s\S]*?\.monitor\{min-height:600px;grid-template-rows:minmax\(420px,1fr\) auto\}/,
      "split and narrow layouts must reserve a usable primary-chart height");
    assert.match(chart, /const row = document\.querySelector<HTMLElement>\("\[data-control-designs\]"\)[\s\S]*?row\.append\(handle\)/,
      "graphical tuning controls must live outside the evidence plot they edit");
    assert.doesNotMatch(chart, /plotWrap\.append\(handle\)/,
      "the chart workspace must not move tuning controls back over plot axes");
    assert.match(panel, /\.plot-empty\{[^}]*z-index:3[^}]*background:var\(--vscode-editor-background\)/,
      "empty and unavailable chart explanations must remain visibly above the chart canvas");
    assert.match(chart, /No live samples yet\. Verify the loop signal bindings in Live Values\./,
      "the visible empty state must explain the next recovery action");
    assert.match(panel, /@media\(max-width:600px\)\{\.plot-toolbar\{flex-direction:column;align-items:stretch\}\.plot-tabs,\.plot-actions\{flex-wrap:wrap;overflow-x:visible\}\.design-row\{flex-wrap:wrap;overflow-x:hidden\}\.design-handle\{width:100%;margin-left:0\}/,
      "at 200 percent zoom, every direct plot tab and chart action must remain immediately visible without horizontal scrolling");
    assert.match(panel, /@media\(max-width:600px\)\{\.design-handle\{display:grid;grid-template-columns:minmax\(0,1fr\) 68px auto/,
      "the exact graphical editor must use a bounded compact grid at 200 percent zoom");
    assert.match(panel, /@media\(min-width:601px\) and \(max-width:720px\)\{\.analysis-grid:not\(\.bode-grid\)\{grid-template-columns:1fr 1fr\}\}/,
      "the split layout may preserve other analysis grids without separating the coordinated Bode figure");
  });

  test("labels live plots with real units and marks only saturation transitions", () => {
    const contract = chartContractModule as unknown as {
      controlChartAxisLabels?: (processUnit: string, outputUnit: string) => unknown;
      controlChartSaturationEvent?: (
        previous: "minimum" | "maximum" | "none" | undefined,
        current: "minimum" | "maximum" | "none"
      ) => unknown;
      controlOutputLimitLines?: (minimum: number, maximum: number) => unknown;
      controlOutputAxisRange?: (minimum: number, maximum: number) => unknown;
      identificationOutputAxisRange?: (values: readonly number[]) => unknown;
    };
    assert.strictEqual(typeof contract.controlChartAxisLabels, "function",
      "live chart unit labels are not implemented");
    assert.deepStrictEqual(contract.controlChartAxisLabels?.("°C", "%"), {
      process: "SP / PV (°C)", output: "MV (%)",
      description: "SP and PV in °C; MV in %",
    });
    assert.deepStrictEqual(contract.controlChartAxisLabels?.("degC", "%"), {
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
    assert.strictEqual(typeof contract.controlOutputLimitLines, "function",
      "output-limit chart evidence is not implemented");
    assert.deepStrictEqual(contract.controlOutputLimitLines?.(0, 100), [
      { name: "Output minimum", value: 0 }, { name: "Output maximum", value: 100 },
    ]);
    assert.throws(() => contract.controlOutputLimitLines?.(100, 0), /minimum.*maximum/i);
    assert.strictEqual(typeof contract.controlOutputAxisRange, "function",
      "the MV axis must use the declared engineering limits instead of normalizing live data");
    assert.deepStrictEqual(contract.controlOutputAxisRange?.(0, 100), { minimum: 0, maximum: 100 });
    assert.deepStrictEqual(contract.controlOutputAxisRange?.(-10, 25), { minimum: -10, maximum: 25 });
    assert.throws(() => contract.controlOutputAxisRange?.(100, 0), /minimum.*maximum/i);
    assert.strictEqual(typeof contract.identificationOutputAxisRange, "function",
      "identification plots must focus on measured model variation instead of forcing zero");
    assert.deepStrictEqual(contract.identificationOutputAxisRange?.([30, 30.5, 31, 31.5]), {
      minimum: 29.88, maximum: 31.62,
    });
    assert.deepStrictEqual(contract.identificationOutputAxisRange?.([30, 30]), {
      minimum: 29.7, maximum: 30.3,
    });
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    assert.match(chart, /markLine:[\s\S]*?label:\s*\{\s*show:\s*false\s*\}/,
      "output-limit labels must not cover live traces; the adjacent loop state owns their text");
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

  test("keeps unavailable analysis compact and blocks impossible workflow actions", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "workflow.ts"), "utf8");
    assert.match(panel, /\.studio\[data-plot-available=false\]\{grid-template-rows:auto auto minmax\(180px,24vh\)/,
      "a prerequisite message must not reserve a full evidence-chart row");
    for (const attribute of ["data-identify-run", "data-tune-generate", "data-validate-run",
      "data-result-save", "data-result-export-csv", "data-result-export-json"]) {
      assert.match(panel, new RegExp(`${attribute}[^>]*disabled`), `${attribute} must start unavailable`);
    }
    assert.match(chart, /root\.dataset\.plotAvailable = String\(Boolean\(active\?\.available\)\)/,
      "the selected plot availability must drive the compact prerequisite layout");
    assert.match(workflow, /setWorkflowActionAvailability\(\{\s*recording: true\s*\}\)/,
      "stopping a real recording must unlock identification");
    assert.match(workflow, /setWorkflowActionAvailability\(\{\s*model: true\s*\}\)/,
      "accepting a model must unlock candidate generation");
    assert.match(workflow, /setWorkflowActionAvailability\(\{\s*candidate: true\s*\}\)/,
      "creating a candidate must unlock validation");
    assert.match(workflow, /setWorkflowActionAvailability\(\{\s*result: true\s*\}\)/,
      "completed validation must unlock save and export");
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

  test("keeps the owned unit-aware chart summary from being overwritten", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    const adapter = chart.slice(chart.indexOf("class ControlChartAdapter"), chart.indexOf("type DesignSnapshot"));
    assert.match(adapter, /aria:\s*\{\s*enabled:\s*false/,
      "the live chart must own its accessible summary so ECharts cannot replace engineering units");
    assert.match(adapter, /controlChartAccessibleSummary\(/);
  });

  test("resizes engineering charts when a hidden direct plot becomes visible", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    const engineering = chart.slice(chart.indexOf("class EngineeringChartAdapter"), chart.indexOf("class MimoChartAdapter"));
    assert.match(engineering, /addEventListener\("trust-control-studio-plot-shown"/,
      "direct engineering plots must react when their previously hidden host becomes visible");
    // A chart initialized (or last drawn) while hidden cannot be trusted to redraw
    // correctly from a bare resize(); replaying the retained evidence through
    // setEvidence is the same pattern already proven for MimoChartAdapter (finding V-03).
    assert.match(engineering, /requestAnimationFrame\(\(\) => \{[\s\S]*?if \(this\.lastEvidence\) this\.setEvidence\(this\.lastEvidence\);[\s\S]*?else this\.resize\(\);/,
      "engineering canvases must fully re-render (not merely resize) after their previously hidden host becomes visible");
    assert.match(engineering, /private lastEvidence: EngineeringPlotMessageEvidence \| undefined;/,
      "the last qualified evidence must be retained so a hidden-then-shown plot can be replayed");
    const identification = chart.slice(chart.indexOf("class IdentificationChartAdapter"), chart.indexOf("function initEngineeringChart"));
    assert.match(identification, /addEventListener\("trust-control-studio-plot-shown"/,
      "identification plots (Fit, Residuals, Validation data, Poles and zeros) must also react when their previously hidden host becomes visible");
    assert.match(identification, /if \(this\.lastEvidence\) this\.setEvidence\(this\.lastEvidence\);/);
  });

  test("presents magnitude and phase as one coordinated Bode figure", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    const engineering = chart.slice(chart.indexOf("class EngineeringChartAdapter"), chart.indexOf("class MimoChartAdapter"));
    assert.match(panel, /class="analysis-grid bode-grid"/,
      "Bode magnitude and phase need an owned coordinated layout");
    assert.match(engineering, /this\.magnitude\.group = "control-studio-bode"/);
    assert.match(engineering, /this\.phase\.group = "control-studio-bode"/);
    assert.match(engineering, /echarts\.connect\("control-studio-bode"\)/,
      "frequency zoom, pan, and inspection must be linked");
    assert.match(engineering, /this\.phase\.setOption\(\{[\s\S]*?legend:\s*\{\s*show:\s*false\s*\}/,
      "the coordinated figure must not repeat its legend");
    assert.match(panel, /\.bode-grid\{grid-template-columns:1fr;grid-template-rows:minmax\(0,1fr\) minmax\(0,1fr\)/,
      "magnitude and phase must use aligned stacked panels");
  });

  test("validates exact graphical gain edits without losing the last valid candidate", () => {
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
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    assert.match(panel, /data-design-exact="proportionalGain"[^>]*type="number"/,
      "graphical Kp must have a keyboard-exact numeric input");
    assert.match(panel, /data-design-error[^>]*role="status"/,
      "an invalid exact edit needs an adjacent accessible explanation");
    assert.match(chart, /range\.value = exact\.value = String\(value\)/,
      "range and exact controls must stay synchronized");
  });

  test("shows candidate margins from the same engineering evidence update", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    assert.match(panel, /data-design-gain-margin/);
    assert.match(panel, /data-design-phase-margin/);
    const evidence = chart.slice(chart.indexOf("setEvidence(evidence: EngineeringPlotMessageEvidence)"), chart.indexOf("private renderRootLocus"));
    assert.match(evidence, /\[data-design-gain-margin\][\s\S]*?evidence\.gainMarginDb/);
    assert.match(evidence, /\[data-design-phase-margin\][\s\S]*?evidence\.phaseMarginDegrees/);
  });

  test("renders one selectable signal row with identity, value, unit, and quality", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    assert.match(chart, /role\.textContent\s*=\s*`\$\{signal\.role\} · \$\{signal\.symbol\}`/,
      "the flat Signals view must not hide the bound runtime symbol");
    assert.match(chart, /checked\.addEventListener\("change"/);
    assert.match(chart, /formatEngineeringNumber\(signal\.value\)/);
    assert.match(chart, /quality\.textContent\s*=\s*signal\.quality/);
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

  test("rebuilds direct engineering plots when a compatible saved result is reopened", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    assert.match(panel, /async function postRestoredEngineeringPlots/,
      "saved results need an explicit direct-plot restoration path");
    assert.match(panel, /buildEngineeringPlotEvidence\(result\.model, current, result\.candidate\)/,
      "restored plots must use the saved model, current controller, and candidate");
    assert.match(panel, /phase === "restored" \|\| phase === "opened"/);
    assert.match(panel, /await postRestoredEngineeringPlots\(result\)/,
      "plot restoration must occur as part of reopening the saved result");
  });

  test("renders one direct chart workspace without a tools disclosure or plot catalogue", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const panel = readControlStudioPanelSource(extensionRoot);
    const chart = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "chart.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "workflow.ts"), "utf8");
    assert.doesNotMatch(panel, /<details class="plot-tools"/);
    assert.match(panel, /data-control-plot-tabs/);
    assert.match(panel, /data-control-signals/);
    assert.match(panel, /data-control-designs/);
    assert.match(panel, /data-engineering-root-locus/);
    assert.match(panel, /data-engineering-nyquist/);
    assert.match(panel, /data-engineering-nichols/);
    assert.match(panel, /data-engineering-sensitivity/);
    assert.match(panel, /data-engineering-requirements/);
    assert.match(panel, /data-design-handle="proportionalGain"/);
    assert.match(panel, /\.monitor\{display:grid;grid-template-columns:minmax\(0,1fr\);grid-template-rows:auto minmax\(\d+px,1fr\)/,
      "the compact live strip must not take horizontal width away from the dominant plot");
    assert.match(panel, /\.plot-column\{grid-row:2/);
    assert.match(panel, /\.values\{grid-row:1/);
    assert.match(panel, /\.design-handle\[hidden\]\{display:none\}/,
      "graphical tuning controls must not leak into Monitor or other stages");
    const engineeringAdapter = chart.slice(
      chart.indexOf("class EngineeringChartAdapter"),
      chart.indexOf("class IdentificationChartAdapter")
    );
    assert.doesNotMatch(engineeringAdapter, /this\.section\.hidden = false/,
      "legacy engineering evidence must not open a second chart surface");
    assert.match(chart, /controlStagePlots/);
    assert.match(chart, /trust-control-studio-stage/);
    assert.match(workflow, /trust-control-studio-stage/);
    assert.match(chart, /trust-control-studio-restore-stage/);
    assert.match(workflow, /trust-control-studio-restore-stage/);
    assert.match(chart, /selectedSignals: \[\.\.\.this\.selectedSignals\]/,
      "the saved session must retain the user's actual signal selection");
    for (const plot of ["fit", "residuals", "validation-data", "poles-zeros"] as const) {
      assert.match(chart, new RegExp(`installAnalysisView\\(\"${plot}\"`),
        `${plot} must have its own direct analysis view`);
    }
    for (const plot of ["channel-responses", "rga", "singular-values", "interaction"] as const) {
      assert.match(chart, new RegExp(`installMimoView\\(\"${plot}\"`),
        `${plot} must have its own direct MIMO view`);
    }
    assert.match(chart, /control-studio\.mimo\.result/);
    assert.match(chart, /decodeMimoPlotEvidence/);
    assert.match(chart, /new CustomEvent\("trust-control-studio-plot-shown"/,
      "revealing a direct plot must notify adapters that were initialized while hidden");
    assert.match(chart, /addEventListener\("trust-control-studio-plot-shown"/,
      "MIMO charts must resize after their direct plot becomes visible");
    assert.match(chart, /this\.renderEvidence\(this\.evidence\)/,
      "MIMO chart options must be reapplied after a hidden chart becomes visible");
    assert.match(chart, /chart\.resize\(\{ width: rect\.width, height: rect\.height \}\)/,
      "MIMO charts must receive the measured visible host size explicitly");
    assert.match(chart, /const cellHeight = \(76 - gap \* \(rows - 1\)\) \/ rows/,
      "MIMO response grids must reserve space for lower axis labels");
    assert.match(chart, /scrollIntoView\(\{ block: "nearest", inline: "nearest" \}\)/,
      "the selected plot tab must remain visible in the compact toolbar");
    assert.match(chart, /MIMO channel responses for/,
      "MIMO channel evidence needs an owned accessible summary");
    assert.match(chart, /installGraphicalDesignHandle/,
      "Bode, root-locus, and Nichols must share the same direct candidate handle");
  });
});
