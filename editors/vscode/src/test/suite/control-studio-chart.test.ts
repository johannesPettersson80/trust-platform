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
import { recordedResponseChannels } from "../../controlStudio/webview/engineerRecordedResponses";
import { commissioningRecordedResponse } from "../../controlStudio/webview/commissioningRecordedResponse";
import { ControlChartWorkspace } from "../../controlStudio/webview/chartWorkspace";
import { liveSignalPlot, liveSignals } from "../../controlStudio/webview/liveSignalPlot";

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
  test("projects an explicit open-loop plot without inventing a setpoint", () => {
    const samples = [{ elapsedSeconds: 0, sp: null, pv: 35, mv: 85, additional: [
      { id: "experiment-authority", channel: "experiment", role: "state", unit: "", value: 1, quality: "good" as const },
    ] }];
    const openLoopSignals = (liveSignals as unknown as (
      values: typeof samples, processUnit: string, outputUnit: string, topology?: "closed-loop" | "open-loop"
    ) => ReturnType<typeof liveSignals>)(samples, "°C", "%", "open-loop");
    const theme = { foreground: "black", muted: "gray", border: "gray", background: "white",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "gold" };
    assert.deepStrictEqual(openLoopSignals.map(signal => signal.id), ["pv", "mv", "experiment-authority"],
      "an open-loop process has measured output and applied input, with no fabricated SP");
    assert.deepStrictEqual(openLoopSignals.slice(0, 2).map(signal => signal.name), ["Measured output", "Applied input"]);
    const all = liveSignalPlot(samples, openLoopSignals, new Set(openLoopSignals.map(signal => signal.id)),
      { width: 1280, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme);
    assert.deepStrictEqual(all.legend.data, ["Measured output", "Applied input", "Experiment authority"]);
    assert.ok(!all.series.some(series => series.name === "SP"), "the open-loop option must not contain an invented setpoint series");
    const auxiliary = all.series.find(series => series.name === "Experiment authority");
    assert.strictEqual(auxiliary?.itemStyle.color, theme.auxiliaryOrange,
      "removing SP must not shift the first auxiliary trace to a negative palette index");
    const primary = liveSignalPlot(samples, openLoopSignals, new Set(["pv", "mv"]),
      { width: 1280, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme);
    assert.ok(primary.yAxis.some(axis => axis.name === "Measured output (°C)"));
    assert.ok(primary.yAxis.some(axis => axis.name === "Applied input (%)"));
  });
  test("labels an isolated controller-state axis by its actual role", () => {
    const samples = [{ elapsedSeconds: 0, sp: 45, pv: 39, mv: 100, additional: [
      { id: "tracking-correction", channel: "tracking", role: "state", unit: "percentage points", value: -0.2, quality: "good" as const },
    ] }];
    const signals = liveSignals(samples, "°C", "%");
    const theme = { foreground: "black", muted: "gray", border: "gray", background: "white",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "gold" };
    const option = liveSignalPlot(samples, signals, new Set(["sp", "pv", "tracking-correction"]),
      { width: 1280, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme);
    assert.strictEqual(option.yAxis[1].name, "Tracking correction (percentage points)",
      "the right axis represents recorded controller state, not process PV");
  });
  test("keeps auxiliary trace identities distinct from primary signals and stable across selections", () => {
    const samples = [{ elapsedSeconds: 0, sp: 45, pv: 39, mv: 100, additional:
      Array.from({ length: 9 }, (_, index) => ({ id: `state-${index}`, channel: `state-${index}`,
        role: "state", unit: "%", value: index, quality: "good" as const })),
    }];
    const signals = liveSignals(samples, "°C", "%");
    const theme = { foreground: "black", muted: "gray", border: "gray", background: "white",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "gold" };
    const make = (selected: string[]) => liveSignalPlot(samples, signals, new Set(selected),
      { width: 1280, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme);
    const all = make(signals.map(signal => signal.id));
    const identity = (series: typeof all.series[number]) => `${series.lineStyle.color}/${series.lineStyle.type}`;
    assert.strictEqual(new Set(all.series.map(identity)).size, signals.length,
      "the included diagnostic traces must not reuse each other's or the primary traces' colour/style pair");
    const isolated = make(["sp", "pv", "state-3"]);
    assert.deepStrictEqual(isolated.series.map(identity), all.series.map(identity),
      "signal selection must not reassign the remaining trace identities");
  });
  test("reserves a legend header above the response lanes and their axis labels", () => {
    const samples = [{ elapsedSeconds: 0, sp: 45, pv: 39, mv: 100, additional: [
      { id: "requested-power", channel: "requested-power", role: "state", unit: "%", value: 125, quality: "good" as const },
      { id: "unavailable-power", channel: "unavailable-power", role: "state", unit: "percentage points", value: 25, quality: "good" as const },
    ] }];
    const signals = liveSignals(samples, "°C", "%");
    const theme = { foreground: "black", muted: "gray", border: "gray", background: "white",
      sp: "green", pv: "blue", mv: "red", auxiliaryOrange: "orange", auxiliaryPurple: "purple", auxiliaryYellow: "gold" };
    const option = liveSignalPlot(samples, signals, new Set(signals.map(signal => signal.id)),
      { width: 1280, height: 500 }, { minimum: 0, maximum: 100 }, "%", theme);
    const legendTop = (option.legend as { top?: number }).top;
    assert.strictEqual(typeof legendTop, "number", "the scroll legend must explicitly occupy the reserved header");
    assert.ok(legendTop! >= 0 && legendTop! + 24 < option.grid[0].top - 15,
      "legend text must fit above the first lane's axis heading instead of overlapping time ticks");
    const compact = liveSignalPlot(samples, signals, new Set(signals.map(signal => signal.id)),
      { width: 1280, height: 300 }, { minimum: 0, maximum: 100 }, "%", theme);
    assert.ok(compact.grid[0].height / compact.yAxis[0].splitNumber >= 18,
      "the compact Test plot must leave a readable line height between y-axis tick labels");
  });
  test("applies saved plot selection without requiring a current monitor sample", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<div data-control-studio></div>');
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      let applied: string[] = [];
      const owner = Object.assign(Object.create(ControlChartWorkspace.prototype), {
        root: dom.window.document.querySelector("div"), selectedPlots: {}, revision: 0,
        sessionBoundary: () => "saved-loop", renderSignals: () => {}, renderDesigns: () => {}, renderTabs: () => {},
        adapter: { restoreWindow: () => {}, setSelectedSignals: (ids: readonly string[]) => { applied = [...ids]; } },
      });
      owner.restoreSession({ schemaVersion: 1, boundary: "saved-loop", activeStage: "Monitor",
        selectedPlots: { Monitor: "response" }, selectedSignals: ["sp", "pv", "requested-power"],
        window: [0, 100], expanded: false, pinnedCandidateRevisions: [] });
      assert.deepStrictEqual(applied, ["sp", "pv", "requested-power"],
        "historical plot selection must reach its adapter even without any current monitor declaration");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
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
  test("commissioning recordings retain explicit SP PV MV identities despite output-only bounds", () => {
    const channels = [{ id: "target", unit: "m" }, { id: "position", unit: "m" }, { id: "drive", unit: "V" }];
    const evaluation = commissioningRecordedResponse({ status: "passed", modelRevision: "model", channels,
      recording: { evidence: { programRevision: "program" }, rawSamples: [
        { elapsedSeconds: 0, sp: 1, pv: 0, appliedMv: 2, quality: "good", source: "Simulation", outputLimits: { minimum: -20, maximum: 20 } },
        { elapsedSeconds: 1, sp: 1, pv: 0.9, appliedMv: 0.1, quality: "good", source: "Simulation", outputLimits: { minimum: -20, maximum: 20 } },
      ] } } as any)!;
    assert.deepStrictEqual(recordedResponseChannels(evaluation.run, evaluation), {
      setpoint: channels[0], processVariable: channels[1], manipulatedVariable: channels[2],
    }, "all three actual recorded curves must have their original units and values");
    assert.strictEqual(evaluation.run.samples[1].values.position.value, 0.9);
    assert.strictEqual(evaluation.specification!.length, 1, "display roles must not invent verification requirements");
  });
  test("resolves recorded SP, PV, and MV roles from requirement identities when channels are permuted", () => {
    const run = {
      channels: [{ id: "Main.MV", unit: "V" }, { id: "Main.PV", unit: "m" }, { id: "Main.SP", unit: "m" }],
      samples: [], evidence: "ST-executed", conditions: { modelRevision: "model", programRevision: "program" },
    } as any;
    const resolved = recordedResponseChannels(run, { specification: [
      { id: "target-error", kind: "error", channel: "Main.PV", reference: { channel: "Main.SP" }, window: { start: 0, end: 1 } },
      { id: "output-bounds", kind: "bounds", channel: "Main.MV", minimum: 0, maximum: 100, window: { start: 0, end: 1 } },
    ] });
    assert.strictEqual(resolved.setpoint?.id, "Main.SP");
    assert.strictEqual(resolved.processVariable?.id, "Main.PV");
    assert.strictEqual(resolved.manipulatedVariable?.id, "Main.MV");
  });

  test("uses positional roles only for a legacy three-channel record without role metadata", () => {
    const run = { channels: [{ id: "sp", unit: "m" }, { id: "pv", unit: "m" }, { id: "mv", unit: "V" }] } as any;
    const resolved = recordedResponseChannels(run, { specification: [] });
    assert.deepStrictEqual(Object.fromEntries(Object.entries(resolved).map(([key, channel]) => [key, channel?.id])), {
      setpoint: "sp", processVariable: "pv", manipulatedVariable: "mv",
    });
    const unknown = recordedResponseChannels(run, { specification: [{ kind: "bounds", channel: "pv", window: { start: 0, end: 1 } }] });
    assert.strictEqual(unknown.setpoint, undefined);
    assert.strictEqual(unknown.manipulatedVariable, undefined);
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
    const chart = readControlChartSource(extensionRoot);
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

  test("keeps the owned unit-aware chart summary from being overwritten", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = readControlChartSource(extensionRoot);
    const adapter = chart.slice(chart.indexOf("class ControlChartAdapter"), chart.indexOf("type DesignSnapshot"));
    assert.match(adapter, /aria:\s*\{\s*enabled:\s*false/,
      "the live chart must own its accessible summary so ECharts cannot replace engineering units");
    assert.match(adapter, /controlChartAccessibleSummary\(/);
  });

  test("resizes engineering charts when a hidden direct plot becomes visible", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = readControlChartSource(extensionRoot);
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

  test("renders one selectable signal row with identity, value, unit, and quality", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const chart = readControlChartSource(extensionRoot);
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


function readControlChartSource(extensionRoot: string): string {
  return ["chart.ts", "liveChart.ts", "liveSignalPlot.ts", "chartWorkspace.ts", "engineeringCharts.ts", "chartPrimitives.ts"]
    .map(file => fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", file), "utf8")).join("\n");
}
