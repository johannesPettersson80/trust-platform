import * as assert from "assert";
import { retainedPeriodicRecordingSource } from "../../controlStudio/periodicRecordingSource";
import { completedExperimentRecording, decodePeriodicRecording } from "../../controlStudio/webview/periodicRecording";
import type { DeclaredControlAnalysis } from "../../controlStudio/declaredAnalysis";
import { calculateGeneralStateStructure } from "../../controlStudio/generalStateStructure";
import type { GeneralStateSpaceModel } from "../../controlStudio/generalModel";
import { stateStructureDetails } from "../../controlStudio/webview/stateStructureDetails";
import { formatNumberList } from "../../controlStudio/webview/numberListText";
import { renderIdentifiedModel } from "../../controlStudio/webview/threePageModel";

const declaredFixture: DeclaredControlAnalysis = {
  plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1, 0], delaySeconds: 0,
    inputUnit: "N·m", outputUnit: "rad", revision: "shaft-j1-b1", source: "provided", validation: "unvalidated" },
  controller: { numerator: [{ parameter: "Kp" }], denominator: [1], parameters: { Kp: { value: 2, minimum: 0, maximum: 8, unit: "N·m/rad" } } },
  rootLocus: { parameter: "Kp", values: [0, 2, 4, 8], selectedValue: 2 },
};

/** This test intentionally loads the new presentation owner dynamically so the
 * pre-fix run reaches the behavior assertion even before the module exists. */
suite("Control Studio three-page analysis presentation", () => {
  test("provides one externally selected contextual analysis disclosure", () => {
    let render: (() => string) | undefined;
    try {
      render = (require("../../controlStudio/threePageAnalysisTemplate") as { renderThreePageAnalysisTemplate?: () => string }).renderThreePageAnalysisTemplate;
    } catch {
      // The expected pre-fix failure is the assertion below, not a module-load failure.
    }
    const markup = render?.() ?? "";
    assert.match(markup, /data-three-page-analysis/);
    assert.doesNotMatch(markup, /data-three-page-analysis-tool/);
    assert.doesNotMatch(markup, /provided model|read-only/i);
    assert.match(markup, /<details[^>]*data-three-page-analysis-table-details[^>]*hidden><summary>Calculation details<\/summary>/);
    assert.deepStrictEqual(declaredFixture.rootLocus.values, [0, 2, 4, 8], "the selected declared sweep remains a typed fixture");
  });

  test("presents the state structure with the declared order, ranks out of the state count, matrices in one precision and the scope", () => {
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -0.05, maximum: 0.05 } });
    const model = { kind: "continuous-state-space", revision: "cascade", source: "provided", validation: "validated", evidence: [], delaySeconds: 0,
      inputs: [channel("pump-flow-deviation", "m3/s")], outputs: [channel("downstream-level-deviation", "m")],
      states: [channel("upstream-level-deviation", "m"), channel("downstream-level-deviation", "m")],
      a: [[-0.04, 0], [0.04, -0.04]], b: [[2], [0]], c: [[0, 1]], d: [[0]] } as unknown as GeneralStateSpaceModel;
    const details = stateStructureDetails(model, calculateGeneralStateStructure(model));
    assert.strictEqual(details.summary, "Controllability rank 2 of 2; observability rank 2 of 2.");
    assert.strictEqual(details.order, "State order: upstream-level-deviation (m), downstream-level-deviation (m). Input order: pump-flow-deviation (m3/s). Output order: downstream-level-deviation (m).");
    assert.deepStrictEqual(details.matrices, [
      ["A", "[-0.04, 0]; [0.04, -0.04]"], ["B", "[2]; [0]"], ["C", "[0, 1]"], ["D", "[0]"],
      ["Controllability [B, AB, …]", "[2, -0.08]; [0, 0.08]"], ["Observability [C; CA; …]", "[0, 1]; [0.04, -0.04]"],
    ]);
    assert.match(details.scope, /local unconstrained model/);
  });

  test("lists Routh entries and cancelled factors with one precision", () => {
    assert.strictEqual(formatNumberList([12.345, 0.25, 7]), "12.345, 0.25, 7");
    assert.strictEqual(formatNumberList([1, 0.08, 0.0016]), "1, 0.08, 0.0016");
    assert.strictEqual(formatNumberList([]), "");
  });

  test("shows the measured phase unwrapped across the tones, as the model's Bode plot does", () => {
    let rows: ((points: readonly any[]) => { frequency: number; phaseDegrees: number }[]) | undefined;
    try {
      rows = (require("../../controlStudio/measuredFrequencyRows") as { measuredFrequencyRows?: typeof rows }).measuredFrequencyRows;
    } catch {
      // The expected pre-fix failure is the assertion below, not a module-load failure.
    }
    const point = (frequencyRadPerSecond: number, degrees: number) => ({ frequencyRadPerSecond, magnitude: 0.01, phaseRadians: degrees * Math.PI / 180, coherence: 1 });
    // A resonance turns the phase through −180°: atan2 alone wraps −189.4° to +170.6°.
    const shown = rows?.([point(12.566, 170.6), point(6.283, -176.6), point(18.85, 82.35), point(25.133, -3.41)]) ?? [];
    assert.deepStrictEqual(shown.map(row => row.frequency), [6.283, 12.566, 18.85, 25.133], "the tones in ascending frequency");
    assert.deepStrictEqual(shown.map(row => Number(row.phaseDegrees.toFixed(2))), [-176.6, -189.4, -277.65, -363.41]);
  });

  test("counts a model's channels in the singular for one input or output", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<main><p data-studio-model-type></p><pre data-studio-model-equation></pre><section data-studio-model-result hidden></section></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 1 } });
      renderIdentifiedModel(root, { kind: "continuous-state-space", revision: "r", source: "provided", validation: "validated", evidence: ["e"],
        delaySeconds: 0, inputs: [channel("pump", "m3/s")], outputs: [channel("h1", "m"), channel("h2", "m")], states: [channel("x1", "m"), channel("x2", "m")],
        a: [[-0.04, 0], [0.04, -0.04]], b: [[2], [0]], c: [[1, 0], [0, 1]], d: [[0], [0]] } as any);
      assert.strictEqual(root.querySelector("[data-studio-model-type]").textContent, "Continuous state-space model · 1 input, 2 outputs");
    } finally { dom.window.close(); }
  });

  test("writes powers of s as superscripts in the identified transfer function", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<main><p data-studio-model-type></p><pre data-studio-model-equation></pre><section data-studio-model-result hidden></section></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 1 } });
      renderIdentifiedModel(root, { kind: "transfer-function", revision: "r", source: "provided", validation: "validated", evidence: ["e"],
        numerator: [2], denominator: [6, 5, 1], delaySeconds: 0, inputs: [channel("u", "V")], outputs: [channel("y", "m")] } as any);
      // The equation is a stacked fraction: its numerator and denominator are read separately.
      assert.strictEqual(root.querySelector(".studio-math-num").textContent, "2");
      assert.strictEqual(root.querySelector(".studio-math-den").textContent, "6s² +5s +1");
    } finally { dom.window.close(); }
  });

  test("aligns analysis table headers with their cells and separates the columns", () => {
    const { JSDOM } = require("jsdom");
    const fs = require("fs"), path = require("path");
    const theme = fs.readFileSync(path.resolve(__dirname, "../../../src/webview/theme.css"), "utf8");
    const dom = new JSDOM(`<html><head><style>${theme}</style></head><body><table class="trust-table"><thead><tr><th>Stage</th><th>Gain K</th></tr></thead>
      <tbody><tr><td>0</td><td>[123456.7, 234567.8]</td></tr></tbody></table></body></html>`);
    try {
      const header = dom.window.document.querySelector("th"), cell = dom.window.document.querySelector("td");
      const style = (element: Element) => dom.window.getComputedStyle(element);
      assert.strictEqual(style(header).textAlign, "left", "a header sits over its left-aligned column");
      assert.strictEqual(style(cell).textAlign, "left");
      assert.ok(parseFloat(style(cell).paddingRight) >= 12, `columns are separated, not touching: ${style(cell).paddingRight}`);
    } finally { dom.window.close(); }
  });

  test("keeps automatic as the default and exposes no runtime-write action", () => {
    let markup = "";
    try {
      const { renderThreePageAnalysisTemplate } = require("../../controlStudio/threePageAnalysisTemplate") as {
        renderThreePageAnalysisTemplate?: () => string;
      };
      markup = renderThreePageAnalysisTemplate?.() ?? "";
    } catch {
      // The expected pre-fix failure is the assertion below, not a module-load failure.
    }
    assert.match(markup, /hidden/);
    assert.doesNotMatch(markup, /data-.*(?:apply|write|runtime)/i);
    assert.doesNotMatch(markup, /Calculate|Import model|Parameter value/i);
  });

  test("measures the cycles of a completed project experiment", () => {
    // A live recording: the first scan has no previous scan, so its cycle time is unknown; every later scan took 0.1 s.
    const rawSamples = Array.from({ length: 64 }, (_, index) => ({ scan: 100 + index, quality: "good", loopRevision: "loop-1",
      actualCycleSeconds: index === 0 ? null : 0.1, appliedMv: 75 + 5 * Math.sin(0.2 * index * 0.1), pv: 35,
      outputLimits: { minimum: 0, maximum: 100 } }));
    const recording = { rawSamples, evidence: { completeness: "complete", missingScanRanges: [], droppedObservedSamples: 0,
      actualCycleTimingAvailable: true, recordingId: "project-run" } };
    const message = { kind: "control-studio.experiment.result", phase: "terminal", payload: { status: "completed", recording } };
    assert.deepStrictEqual(completedExperimentRecording(message), recording, "the completed experiment's recording is offered to the analyses");
    assert.strictEqual(completedExperimentRecording({ ...message, phase: "running" }), undefined);
    assert.strictEqual(completedExperimentRecording({ ...message, payload: { status: "aborted", recording } }), undefined);
    const decoded = decodePeriodicRecording({ recording });
    assert.ok(decoded, "an unknown first cycle time does not disqualify the recording");
    assert.strictEqual(decoded!.sampleTimeSeconds, 0.1);
    assert.strictEqual(decoded!.input.length, 64);
    assert.strictEqual(decoded!.recordingId, "project-run");
    const irregular = { ...recording, rawSamples: rawSamples.map((sample, index) => index === 30 ? { ...sample, actualCycleSeconds: 0.2 } : sample) };
    assert.strictEqual(decodePeriodicRecording({ recording: irregular }), undefined, "a later scan with another cycle time still disqualifies it");
    // The host measures the same retained recording.
    const source = retainedPeriodicRecordingSource(recording as never, "project-run", "loop-1");
    assert.ok(source, "the host accepts the retained project recording");
    assert.strictEqual(source!.sampleTimeSeconds, 0.1);
    assert.strictEqual(retainedPeriodicRecordingSource(irregular as never, "project-run", "loop-1"), undefined);
  });

  test("measures a sine about an operating point without the operating point leaking into the estimate", () => {
    const operations = require("../../controlStudio/threePageAnalysisOperations") as { estimatePeriodicFrequency: Function };
    // The heater at 0.2 rad/s: command 75 + 5·sin(0.2t) %, temperature 35 + 0.7071·sin(0.2t − 0.9854) °C. A period is
    // 314.16 samples of 0.1 s, so no segment holds a whole number of periods.
    const input = Array.from({ length: 942 }, (_, index) => 75 + 5 * Math.sin(0.2 * index * 0.1));
    const output = input.map((_, index) => 35 + 5 * 0.141421 * Math.sin(0.2 * index * 0.1 - 0.985398));
    for (const segmentCount of [2, 3]) {
      const measured = operations.estimatePeriodicFrequency(input, output, 0.1, [0.2], { segmentCount, minimumCoherence: 0.9 });
      assert.strictEqual(measured.status, "qualified", JSON.stringify(measured));
      assert.ok(Math.abs(measured.points[0].magnitude - 0.141421) <= 0.0005, `${segmentCount} segments: ${JSON.stringify(measured.points[0])}`);
      assert.ok(Math.abs(measured.points[0].phaseRadians + 0.985398) <= 0.005, `${segmentCount} segments: ${JSON.stringify(measured.points[0])}`);
    }
    // Discarding the start leaves whatever number of samples the record has: the last few that do not fill a
    // segment are left out rather than the whole record refused.
    const trimmed = operations.estimatePeriodicFrequency(input.concat(input.slice(0, 1)), output.concat(output.slice(0, 1)), 0.1, [0.2],
      { segmentCount: 3, minimumCoherence: 0.9, startupSamples: 0 });
    assert.strictEqual(trimmed.status, "qualified", JSON.stringify(trimmed));
  });

  test("exposes bounded periodic measurement and lead-lag owner seams", () => {
    let operations: { estimatePeriodicFrequency?: Function; calculateLeadLag?: Function } | undefined;
    try { operations = require("../../controlStudio/threePageAnalysisOperations") as typeof operations; } catch { operations = undefined; }
    assert.strictEqual(typeof operations?.estimatePeriodicFrequency, "function");
    assert.strictEqual(typeof operations?.calculateLeadLag, "function");
    const input = Array.from({ length: 64 }, (_, index) => Math.sin(index * 0.1 * 5));
    const measured = operations!.estimatePeriodicFrequency!(input, input.map(value => 2 * value), 0.1, [5], { segmentCount: 2, minimumCoherence: 0 });
    assert.strictEqual(measured.status, "qualified");
    const lead = operations!.calculateLeadLag!({ numerator: [1], denominator: [1, 1], delaySeconds: 0 }, "lead", 1, 30);
    assert.strictEqual(lead.status, "qualified");
  });

});
