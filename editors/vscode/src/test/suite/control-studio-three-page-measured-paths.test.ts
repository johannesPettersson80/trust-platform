import * as assert from "assert";
import type { AnalysisEnvelope, ToolOffer } from "../../controlStudio/generated/AnalyzeAnswer";
import { ANALYSIS_KIND } from "../../controlStudio/webview/engineAnalysis";
import {
  initializeThreePageMimoAnalysis, routeMeasuredPaths, type ThreePageMimoAnalysisPresenter,
} from "../../controlStudio/webview/threePageMimoAnalysis";
import { renderThreePageMimoAnalysisTemplate } from "../../controlStudio/threePageMimoAnalysisTemplate";

/**
 * The measured-paths section of a served loop (specification 35 C7; lessons 61 and 62): it draws the engine's `measuredPaths` answer and
 * sends the windows the engineer entered. The answers below are fixtures of the engine's shape; the Rust tests hold their numbers.
 */

const CONTEXT = { inputIds: ["heater-1", "heater-2"] };
const OFFER = {
  tool: "measuredPaths", label: "Measured MIMO paths", action: "Calculate measured paths", abortAction: "Abort analysis", operation: true, defaults: {},
} as unknown as ToolOffer;
const CHANNELS = {
  inputs: [{ id: "heater-1", symbol: "P1.Heater1Pct", unit: "%" }, { id: "heater-2", symbol: "P1.Heater2Pct", unit: "%" }],
  outputs: [{ id: "vessel-1", symbol: "P1.Vessel1C", unit: "degC" }, { id: "vessel-2", symbol: "P1.Vessel2C", unit: "degC" }],
};
const TRIALS = [
  { inputId: "heater-1", baseline: { startSeconds: 0, endSeconds: 1.9 }, settled: { startSeconds: 27.1, endSeconds: 31.9 } },
  { inputId: "heater-2", baseline: { startSeconds: 57.1, endSeconds: 61.9 }, settled: { startSeconds: 87.1, endSeconds: 92 } },
];
const NAMED_PAIRING = {
  status: "named",
  pairs: [
    { outputId: "vessel-1", inputId: "heater-1", relativeGain: 1.242638 },
    { outputId: "vessel-2", inputId: "heater-2", relativeGain: 1.242638 },
  ],
};
const REFUSED_REASON = "No pairing has a positive relative gain on every diagonal element, so no pairing is named.";

/** The engine's envelope of a qualified `measuredPaths` calculation (C7), with the given pairing. */
function answer(pairing: Record<string, unknown>): AnalysisEnvelope {
  return {
    tool: "measuredPaths",
    inputs: { settledTrials: TRIALS, maximumUnexcitedInputDrift: 0.001 },
    defaults: {},
    result: {
      measuredPaths: {
        recordingId: "recording-1", recordingRevision: "1", ...CHANNELS, trials: TRIALS, sampleCount: 670,
        gainMatrix: [[0.138036, 0.060694], [0.061113, 0.137617]],
        relativeGainArray: [[1.242638, -0.242638], [-0.242638, 1.242638]],
        decoupler: [[0.14, -0.06], [-0.06, 0.14]],
        gainConditionNumber: 1.4, inputRank: 2, inputConditionNumber: 1.1, residualNorm: 0.000012,
        pairing,
        summary: ["670 samples from 2 independent input steps."],
        details: ["The gain matrix's condition number is 1.4."],
      },
    },
  } as unknown as AnalysisEnvelope;
}

type Posted = { kind: string; payload?: Record<string, unknown> };

/** Runs `body` on a fresh page with the section mounted, and with the globals the section reads set to that page. */
function withPage(body: (page: { document: Document; presenter: ThreePageMimoAnalysisPresenter; posted: Posted[]; changes: boolean[] }) => void): void {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main>${renderThreePageMimoAnalysisTemplate()}</main>`);
  const original = { window: globalThis.window, document: globalThis.document };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document });
  try {
    const posted: Posted[] = [];
    const changes: boolean[] = [];
    const presenter = initializeThreePageMimoAnalysis(dom.window.document.querySelector("main") as HTMLElement, CONTEXT,
      (kind, payload) => { posted.push({ kind, payload }); }, selected => { changes.push(selected); });
    body({ document: dom.window.document, presenter, posted, changes });
  } finally {
    Object.assign(globalThis, original);
    dom.window.close();
  }
}

function textOf(document: Document, selector: string): string {
  return document.querySelector(selector)?.textContent ?? "";
}

function fillWindows(document: Document, inputId: string, values: readonly string[]): void {
  ["baseline-start", "baseline-end", "settled-start", "settled-end"].forEach((key, index) => {
    const input = document.querySelector(`[data-mimo-input="${inputId}"][data-mimo-window="${key}"]`) as HTMLInputElement;
    input.value = values[index] ?? "";
  });
}

suite("Control Studio measured paths on a served loop", () => {
  test("draws the engine's gain matrix, relative gain array and named pairing, each matrix in one precision", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer(answer(NAMED_PAIRING));
      const gainCells = Array.from(document.querySelectorAll("[data-three-page-mimo-analysis-gain] tbody td"), cell => cell.textContent ?? "");
      assert.deepStrictEqual(gainCells.filter((_, index) => index % 3 === 0), ["P1.Vessel1C (°C)", "P1.Vessel2C (°C)"]);
      const decimals = gainCells.filter((_, index) => index % 3 !== 0).map(text => (text.split(".")[1] ?? "").length);
      assert.strictEqual(new Set(decimals).size, 1, `one precision for the whole matrix, got ${gainCells.join(" | ")}`);
      const headers = Array.from(document.querySelectorAll("[data-three-page-mimo-analysis-gain] thead th"), cell => cell.textContent ?? "");
      assert.deepStrictEqual(headers.slice(1), ["P1.Heater1Pct (%)", "P1.Heater2Pct (%)"]);
      assert.strictEqual(document.querySelectorAll("[data-three-page-mimo-analysis-rga] tbody tr").length, 2, "the relative gain array is shown");
      assert.match(textOf(document, "[data-three-page-mimo-analysis-rga-details]"), /Relative Gain Array/);
      assert.match(textOf(document, "[data-three-page-mimo-analysis-pairing]"),
        /^Pairing: P1\.Vessel1C ← P1\.Heater1Pct \(relative gain [^)]+\), P1\.Vessel2C ← P1\.Heater2Pct \(relative gain [^)]+\)$/);
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-summary]"), "670 samples from 2 independent input steps.");
      assert.match(textOf(document, "[data-three-page-mimo-analysis-details]"), /condition number is 1\.4/);
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-status]"), "", "the answer is not a status");
    });
  });

  test("shows a refused pairing with its reason, and still answers the matrices", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer(answer({ status: "refused", pairs: [], reason: REFUSED_REASON }));
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-pairing]"), REFUSED_REASON);
      assert.doesNotMatch(textOf(document, "[data-three-page-mimo-analysis-pairing]"), /Pairing:/, "no pair is named");
      assert.strictEqual(document.querySelectorAll("[data-three-page-mimo-analysis-gain] tbody tr").length, 2, "the gain matrix is still shown");
      assert.strictEqual(document.querySelectorAll("[data-three-page-mimo-analysis-decoupler] tbody tr").length, 2, "the decoupler is still shown");
    });
  });

  test("shows a refused calculation's whole sentence under the windows, and no matrix", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      const detail = "Enter finite, ordered baseline and settled windows for every declared input. The settled window of heater-2 ends first.";
      presenter.answer({
        tool: "measuredPaths", inputs: {}, defaults: {},
        failure: { message: "Enter finite, ordered baseline and settled windows for every declared input.", detail, cause: "settings", nextAction: "none" },
      } as unknown as AnalysisEnvelope);
      const refusal = document.querySelector("[data-three-page-mimo-analysis-refusal]") as HTMLElement;
      assert.strictEqual(refusal.hidden, false);
      assert.strictEqual(refusal.textContent, detail);
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-gain]"), "", "a refused calculation shows no matrix as evidence");
    });
  });

  test("sends the engine the windows that were entered, with the drift tolerance, and no incomplete trial", () => {
    withPage(({ document, presenter, posted }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      fillWindows(document, "heater-1", ["0", "1.9", "27.1", "31.9"]);
      fillWindows(document, "heater-2", ["", "", "87.1", "92"]);
      (document.querySelector("[data-three-page-mimo-analysis-drift]") as HTMLInputElement).value = "0.001";
      presenter.run();
      const run = posted.find(message => message.kind === ANALYSIS_KIND.run);
      assert.deepStrictEqual(run?.payload, { tool: "measuredPaths", inputs: { settledTrials: [TRIALS[0]], maximumUnexcitedInputDrift: 0.001 } });
      const recordingField = document.querySelector("[data-three-page-mimo-analysis-recording-field]") as HTMLElement;
      assert.strictEqual(recordingField.hidden, true, "a served loop names no TypeScript recording: the engine takes the study's last one");
    });
  });

  test("calculates again a reopened analysis with the inputs it recorded, and fills its windows", () => {
    withPage(({ document, presenter, posted, changes }) => {
      presenter.tools([OFFER], undefined);
      presenter.restore("measuredPaths", { settledTrials: TRIALS, maximumUnexcitedInputDrift: 0.001 } as never);
      assert.strictEqual(changes[changes.length - 1], true, "the page shows the section the reopened analysis names");
      assert.strictEqual((document.querySelector("[data-mimo-input='heater-2'][data-mimo-window='settled-end']") as HTMLInputElement).value, "92");
      assert.deepStrictEqual(posted.filter(message => message.kind === ANALYSIS_KIND.run).map(message => message.payload),
        [{ tool: "measuredPaths", inputs: { settledTrials: TRIALS, maximumUnexcitedInputDrift: 0.001 } }]);
    });
  });

  test("routes the measured paths' operations and answers to their section, and every other tool to the analysis", () => {
    const seen: string[] = [];
    const recorder = (name: string) => ({
      tools: () => { seen.push(`${name}.tools`); },
      started: (tool: string, operationId: string) => { seen.push(`${name}.started:${tool}:${operationId}`); },
      answer: (envelope: { tool: string }) => { seen.push(`${name}.answer:${envelope.tool}`); },
      refused: (tool: string | undefined) => { seen.push(`${name}.refused:${tool ?? "method"}`); },
      update: (update: { operationId: string; state: string }) => { seen.push(`${name}.update:${update.operationId}:${update.state}`); },
      restore: (tool: string) => { seen.push(`${name}.restore:${tool}`); },
    });
    const route = routeMeasuredPaths(recorder("analysis") as never, recorder("measured") as never);
    route.started("measuredPaths" as never, "operation-1");
    route.started("poleZero" as never, "operation-2");
    route.update({ operationId: "operation-1", state: "running" } as never);
    route.update({ operationId: "operation-2", state: "running" } as never);
    route.answer({ tool: "measuredPaths" } as never);
    route.answer({ tool: "poleZero" } as never);
    route.refused(undefined, { message: "m", cause: "settings", nextAction: "none" } as never);
    assert.deepStrictEqual(seen, [
      "measured.started:measuredPaths:operation-1",
      "analysis.started:poleZero:operation-2",
      "measured.update:operation-1:running",
      "analysis.update:operation-2:running",
      "measured.answer:measuredPaths",
      "analysis.answer:poleZero",
      "measured.refused:method",
      "analysis.refused:method",
    ]);
  });
});
