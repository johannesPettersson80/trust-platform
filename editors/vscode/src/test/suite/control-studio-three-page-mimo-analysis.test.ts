import * as assert from "assert";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { WorkflowResultSample } from "../../controlStudio/workflowResult";
import { calculateRecordedMimoAnalysis, type MimoSettledTrial } from "../../controlStudio/threePageMimoAnalysis";
import { MimoAnalysisSession } from "../../controlStudio/mimoAnalysisSession";
import { renderThreePageMimoAnalysisTemplate } from "../../controlStudio/threePageMimoAnalysisTemplate";
import { decodeMimoAnalysisResult, initializeThreePageMimoAnalysis } from "../../controlStudio/webview/threePageMimoAnalysis";

suite("Control Studio three-page retained MIMO analysis", () => {
  test("derives measured gain matrix, RGA, decoupler and pairing from declared channels", () => {
    const result = calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 });
    assert.strictEqual(result.status, "qualified");
    if (result.status !== "qualified") return;
    assert.deepStrictEqual(result.gainMatrix, [[2, -1], [-0.5, 1.5]]);
    assert.deepStrictEqual(result.rga.map(row => row.map(value => Number(value.toFixed(6)))), [[1.2, -0.2], [-0.2, 1.2]]);
    assert.deepStrictEqual(result.decoupler.map(row => row.map(value => Number(value.toFixed(6)))), [[0.6, 0.4], [0.2, 0.8]]);
    assert.deepStrictEqual(result.pairing.map(pair => [pair.outputId, pair.inputId, pair.status]), [["y1", "u1", "candidate"], ["y2", "u2", "candidate"]]);
  });

  test("rejects quality-invalid retained channels before fitting", () => {
    const invalid = samples().map(sample => ({ ...sample, quality: "bad" as const }));
    const result = calculateRecordedMimoAnalysis(mimoLoop(), invalid, trials(), { maximumUnexcitedInputDrift: 0.02 });
    assert.deepStrictEqual(result, { status: "rejected", reason: "insufficient-quality-valid-mimo-samples" });
  });

  test("rejects a window that straddles the command change instead of averaging both levels", () => {
    const settledAcrossStep = [{ inputId: "u1", baseline: { startSeconds: 0, endSeconds: 4 }, settled: { startSeconds: 3, endSeconds: 9 } }, trials()[1]];
    assert.deepStrictEqual(calculateRecordedMimoAnalysis(mimoLoop(), samples(), settledAcrossStep, { maximumUnexcitedInputDrift: 0.02 }),
      { status: "rejected", reason: "window-straddles-input-change" });
    const baselineAcrossStep = [{ inputId: "u1", baseline: { startSeconds: 2, endSeconds: 6 }, settled: { startSeconds: 7, endSeconds: 9 } }, trials()[1]];
    assert.deepStrictEqual(calculateRecordedMimoAnalysis(mimoLoop(), samples(), baselineAcrossStep, { maximumUnexcitedInputDrift: 0.02 }),
      { status: "rejected", reason: "window-straddles-input-change" });
  });

  test("explains a rejected measured-path analysis in plain words", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement);
      presenter.setSelected(true);
      presenter.receive({ status: "rejected", reason: "window-straddles-input-change" });
      const text = dom.window.document.querySelector("[data-three-page-mimo-analysis]")?.textContent ?? "";
      assert.match(text, /A window contains the input step/);
      assert.doesNotMatch(text, /window-straddles-input-change/, "no internal reason code is shown");
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("rejects duplicate input windows instead of accepting a repeated pairing", () => {
    const duplicate = [...trials(), { inputId: "u1", baseline: { startSeconds: 20, endSeconds: 24 }, settled: { startSeconds: 26, endSeconds: 30 } }];
    assert.deepStrictEqual(calculateRecordedMimoAnalysis(mimoLoop(), samples(), duplicate, { maximumUnexcitedInputDrift: 0.02 }), { status: "rejected", reason: "one-settled-trial-per-input-required" });
  });

  test("renders qualified MIMO evidence only in the contextual disclosure", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement);
      const disclosure = dom.window.document.querySelector("[data-three-page-mimo-analysis]") as HTMLElement;
      assert.strictEqual(disclosure.hidden, true);
      presenter.receive(calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 }));
      assert.strictEqual(disclosure.hidden, true, "Retained evidence must not open a second permanent workspace");
      presenter.setSelected(true);
      assert.strictEqual(disclosure.hidden, false);
      assert.match(disclosure.textContent ?? "", /Relative Gain Array/);
      assert.match(disclosure.textContent ?? "", /Static decoupler/);
      presenter.receive({ status: "rejected", reason: "retained recording changed" });
      assert.strictEqual(disclosure.querySelector("[data-three-page-mimo-analysis-gain]")?.textContent, "", "A failed recalculation must not display the previous matrix as current evidence");
      presenter.clear();
      assert.strictEqual(disclosure.hidden, true);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("labels the measured channels in the course's unit notation", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement);
      const measured = calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 });
      assert.strictEqual(measured.status, "qualified");
      if (measured.status !== "qualified") return;
      presenter.setSelected(true);
      presenter.receive({ ...measured, inputs: measured.inputs.map(input => ({ ...input, unit: "%" })), outputs: measured.outputs.map(output => ({ ...output, unit: "degC" })) });
      const summary = dom.window.document.querySelector("[data-three-page-mimo-analysis-summary]")?.textContent ?? "";
      assert.strictEqual(summary, "u1 (%) · u2 (%) → y1 (°C) · y2 (°C)");
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("shows every entry of one matrix with the same precision, so a relative gain row visibly sums to one", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement);
      const measured = calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 });
      assert.strictEqual(measured.status, "qualified");
      if (measured.status !== "qualified") return;
      presenter.setSelected(true);
      presenter.receive({ ...measured, rga: [[1.2427, -0.2427], [-0.2427, 1.2427]] });
      const cells = Array.from(dom.window.document.querySelectorAll("[data-three-page-mimo-analysis-rga] tbody td"), cell => (cell as HTMLElement).textContent);
      assert.deepStrictEqual(cells, ["y1", "1.2427", "-0.2427", "y2", "-0.2427", "1.2427"]);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("keeps each matrix's shared precision visible, including trailing zeros", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement);
      const measured = calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 });
      assert.strictEqual(measured.status, "qualified");
      if (measured.status !== "qualified") return;
      presenter.setSelected(true);
      presenter.receive({ ...measured, gainMatrix: [[0.138, 0.0607], [0.0611, 0.1376]], decoupler: [[9, -3.97], [-4, 9.03]] });
      const cells = (selector: string) => Array.from(dom.window.document.querySelectorAll(`${selector} tbody td`), cell => (cell as HTMLElement).textContent);
      assert.deepStrictEqual(cells("[data-three-page-mimo-analysis-gain]"), ["y1", "0.1380", "0.0607", "y2", "0.0611", "0.1376"]);
      assert.deepStrictEqual(cells("[data-three-page-mimo-analysis-decoupler]"), ["u1", "9.00", "-3.97", "u2", "-4.00", "9.03"]);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("states what the measured paths rest on in plain words", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement);
      const measured = calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 });
      assert.strictEqual(measured.status, "qualified");
      if (measured.status !== "qualified") return;
      presenter.setSelected(true);
      presenter.receive(measured);
      // A qualified fit always has full-rank, well-conditioned input steps; anything else is rejected as poor excitation.
      assert.strictEqual(dom.window.document.querySelector("[data-three-page-mimo-analysis-status]")?.textContent,
        `${measured.sampleCount} good samples from 2 independent input steps.`);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("unwraps the host completion envelope before rendering the measured result", () => {
    const measured = calculateRecordedMimoAnalysis(mimoLoop(), samples(), trials(), { maximumUnexcitedInputDrift: 0.02 });
    const envelope = { status: "complete", result: measured, recordingId: "recording-1", recordingHash: "hash", trials: trials() };
    assert.strictEqual(decodeMimoAnalysisResult(envelope), measured);
    const failed = decodeMimoAnalysisResult({ status: "failed", reason: "retained recording changed" });
    assert.deepStrictEqual(failed, { status: "rejected", reason: "retained recording changed" });
  });

  test("publishes the selected retained recording and explicit settled windows", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document });
    try {
      let request: { recordingId: string; trials: readonly MimoSettledTrial[] } | undefined;
      const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement, {
        inputIds: ["u1", "u2"], recordings: [{ id: "recording-1", label: "Settled test" }],
        onAnalyze: (recordingId, trials) => { request = { recordingId, trials }; },
      });
      presenter.setSelected(true);
      const fields = dom.window.document.querySelectorAll("[data-mimo-window]");
      ["0", "4", "6", "9", "10", "14", "16", "19"].forEach((value, index) => (fields[index] as HTMLInputElement).value = value);
      presenter.run();
      assert.strictEqual(request?.recordingId, "recording-1");
      assert.deepStrictEqual(request?.trials, trials());
      presenter.clear();
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("persists the selected recording identity, hash and exact windows for reopen", () => {
    const recording = { rawSamples: samples(), samples: [], evidence: { recordingId: "recording-1", method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 32, missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: true, reason: "test" } } as any;
    const session = new MimoAnalysisSession();
    const saved = session.analyze(mimoLoop(), recording, "recording-1", trials(), { maximumUnexcitedInputDrift: 0.02 });
    assert.strictEqual(session.restore(mimoLoop(), recording, saved), true);
    const changed = { ...recording, rawSamples: recording.rawSamples.slice(1) };
    assert.strictEqual(session.restore(mimoLoop(), changed, saved), false);
  });
});

function trials(): MimoSettledTrial[] {
  return [
    { inputId: "u1", baseline: { startSeconds: 0, endSeconds: 4 }, settled: { startSeconds: 6, endSeconds: 9 } },
    { inputId: "u2", baseline: { startSeconds: 10, endSeconds: 14 }, settled: { startSeconds: 16, endSeconds: 19 } },
  ];
}

function mimoLoop(): ControlLoopDefinition {
  return {
    schemaVersion: 2, id: "mimo-loop", name: "MIMO", revision: "r1", projectId: "p", runtimeId: "sim",
    controller: { instance: "P1.Controller", type: "Project", design: "project", revision: "c1", action: "direct", taskPeriodMs: 100 },
    signals: {
      processVariable: signal("y1", "y1", "pp"), manipulatedVariable: signal("u1", "u1", "pp"),
      additional: [signal("y2", "y2", "pp", "PV"), signal("u2", "u2", "pp", "MV")],
    },
    channels: {
      inputs: [{ modelChannelId: "u1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: -10, maximum: 10 } },
        { modelChannelId: "u2", signalId: "u2", actuator: { kind: "continuous", minimum: -10, maximum: 10 } }],
      outputs: [{ modelChannelId: "y1", signalId: "processVariable" }, { modelChannelId: "y2", signalId: "y2" }],
    },
    limits: { outputMinimum: -10, outputMaximum: 10 }, observedSampleTiming: { nominalPeriodMs: 100 },
  } as ControlLoopDefinition;
}

function signal(symbol: string, id: string, unit: string, role?: "PV" | "MV") {
  return { ...(role ? { id } : {}), ...(role ? { channel: id, role } : {}), symbol, address: `%${symbol}`, iecType: "LREAL", direction: "read" as const,
    unit, range: { minimum: -100, maximum: 100 } } as any;
}

function samples(): WorkflowResultSample[] {
  return Array.from({ length: 32 }, (_, index) => index < 5 ? sample(index, 0, 0, 0, 0)
    : index < 10 ? sample(index, 1, 2, 0, -0.5)
    : index < 15 ? sample(index, 0, 0, 0, 0)
    : index < 20 ? sample(index, 0, -1, 1, 1.5)
    : sample(index, 0, 0, 0, 0));
}

function sample(index: number, u1: number, y1: number, u2: number, y2: number): WorkflowResultSample {
  return { elapsedSeconds: index, observedAtMs: index * 1000, actualCycleSeconds: 1, scan: index + 1, source: "Simulation", loopRevision: "r1",
    sp: null, pv: y1, requestedMv: u1, appliedMv: u1, units: { sp: "pp", pv: "pp", mv: "pp" }, quality: "good", freshnessMs: 0,
    controllerMode: "Manual", outputLimits: { minimum: -10, maximum: 10 }, rateLimits: {}, safetyState: "Good", events: [],
    additional: [{ id: "y2", channel: "y2", role: "PV", unit: "pp", value: y2, quality: "good" }, { id: "u2", channel: "u2", role: "MV", unit: "pp", value: u2, quality: "good" }] };
}
