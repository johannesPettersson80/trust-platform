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
/** The engine's refusal of heater 2's trial, whose baseline window is blank: its sentence, and the control its field names (C7). */
const MISSING_BASELINE = {
  message: "Enter the baseline start and baseline end of input heater-2.", cause: "settings", nextAction: "none", field: "heater-2:baseline-start",
};

/** The engine's envelope of a qualified `measuredPaths` calculation (C7), with the given pairing and any measured values replaced. */
function answer(pairing: Record<string, unknown>, replaced: Record<string, unknown> = {}): AnalysisEnvelope {
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
        ...replaced,
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

  test("keeps its answer through a refresh of the tools and drops it only when the engine lists it as stale (B9.9)", () => {
    // The measured paths read a recording, not the study's model, so the engine does not list them when the model changes: the page keeps no
    // rule of its own that clears them (it cleared them on every model change before).
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer(answer(NAMED_PAIRING));
      presenter.tools([OFFER], undefined, []);
      assert.strictEqual(document.querySelectorAll("[data-three-page-mimo-analysis-gain] tbody tr").length, 2, "nothing is stale: the answer stays");
      presenter.tools([OFFER], undefined, ["poleZero"]);
      assert.strictEqual(document.querySelectorAll("[data-three-page-mimo-analysis-gain] tbody tr").length, 2, "another tool's stale answer leaves it");
      presenter.tools([OFFER], undefined, ["measuredPaths"]);
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-gain]"), "", "the engine lists the measured paths as stale: the answer is dropped");
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

  test("sends the engine every input's row as typed, a blank window end left out, and shows the engine's refusal of it", () => {
    withPage(({ document, presenter, posted }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      fillWindows(document, "heater-1", ["0", "1.9", "27.1", "31.9"]);
      fillWindows(document, "heater-2", ["", "", "87.1", "92"]);
      (document.querySelector("[data-three-page-mimo-analysis-drift]") as HTMLInputElement).value = "0.001";
      presenter.run();
      const run = posted.find(message => message.kind === ANALYSIS_KIND.run);
      // Each entry goes as the engineer typed it. The JSON of the payload has no blank window end, so heater 2's baseline is an empty object.
      assert.deepStrictEqual(JSON.parse(JSON.stringify(run?.payload)), { tool: "measuredPaths", inputs: { settledTrials: [
        { inputId: "heater-1", baseline: { startSeconds: "0", endSeconds: "1.9" }, settled: { startSeconds: "27.1", endSeconds: "31.9" } },
        { inputId: "heater-2", baseline: {}, settled: { startSeconds: "87.1", endSeconds: "92" } }], maximumUnexcitedInputDrift: "0.001" } });
      presenter.answer({ tool: "measuredPaths", inputs: {}, defaults: {}, failure: MISSING_BASELINE } as unknown as AnalysisEnvelope);
      const refusal = document.querySelector("[data-three-page-mimo-analysis-refusal]") as HTMLElement;
      assert.strictEqual(refusal.hidden, false);
      assert.strictEqual(refusal.textContent, MISSING_BASELINE.message);
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

  test("keeps each matrix's shared precision visible, including trailing zeros", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer(answer(NAMED_PAIRING, { gainMatrix: [[0.138, 0.0607], [0.0611, 0.1376]], decoupler: [[9, -3.97], [-4, 9.03]] }));
      const cells = (selector: string) => Array.from(document.querySelectorAll(`${selector} tbody td`), cell => cell.textContent ?? "");
      assert.deepStrictEqual(cells("[data-three-page-mimo-analysis-gain]"), ["P1.Vessel1C (°C)", "0.1380", "0.0607", "P1.Vessel2C (°C)", "0.0611", "0.1376"]);
      assert.deepStrictEqual(cells("[data-three-page-mimo-analysis-decoupler]"), ["P1.Heater1Pct (%)", "9.00", "-3.97", "P1.Heater2Pct (%)", "-4.00", "9.03"]);
    });
  });

  test("shows the relative gain array in one precision, so a row visibly sums to one", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer(answer(NAMED_PAIRING, { relativeGainArray: [[1.2427, -0.2427], [-0.2427, 1.2427]] }));
      const cells = Array.from(document.querySelectorAll("[data-three-page-mimo-analysis-rga] tbody td"), cell => cell.textContent ?? "");
      assert.deepStrictEqual(cells, ["P1.Vessel1C (°C)", "1.2427", "-0.2427", "P1.Vessel2C (°C)", "-0.2427", "1.2427"]);
    });
  });

  test("an answer does not open the section before the engineer selects it", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.answer(answer(NAMED_PAIRING));
      const section = document.querySelector("[data-three-page-mimo-analysis]") as HTMLElement;
      assert.strictEqual(section.hidden, true, "retained evidence must not open a second permanent workspace");
      presenter.setSelected(true);
      assert.strictEqual(section.hidden, false);
    });
  });

  test("a failed recalculation does not keep the previous matrix as current evidence", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer(answer(NAMED_PAIRING));
      assert.strictEqual(document.querySelectorAll("[data-three-page-mimo-analysis-gain] tbody tr").length, 2);
      presenter.answer({ tool: "measuredPaths", inputs: {}, defaults: {}, failure: MISSING_BASELINE } as unknown as AnalysisEnvelope);
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-gain]"), "", "a failed recalculation shows no matrix as evidence");
    });
  });

  test("shows the engine's sentence of a cancelled calculation, and writes none of its own", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.started("measuredPaths" as never, "operation-1");
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-status]"), "Calculating measured paths…");
      // The cancelled update carries the engine's sentence (`OperationUpdate.message`, B9.4); a page that wrote "Analysis cancelled." itself would show
      // it whatever the engine said, and the words below prove the page repeats the engine's.
      presenter.update({ operationId: "operation-1", state: "cancelled", message: "The engine's words for the cancellation." });
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-status]"), "The engine's words for the cancellation.");
      presenter.started("measuredPaths" as never, "operation-2");
      presenter.update({ operationId: "operation-2", state: "cancelled" });
      assert.strictEqual(textOf(document, "[data-three-page-mimo-analysis-status]"), "", "a cancelled update without a sentence shows none");
    });
  });

  test("restores a blank window end as a blank field, not as the word undefined", () => {
    withPage(({ document, presenter }) => {
      presenter.tools([OFFER], undefined);
      presenter.restore("measuredPaths", { settledTrials: [{ inputId: "heater-2", baseline: {}, settled: TRIALS[1].settled }] } as never);
      const field = (key: string) => (document.querySelector(`[data-mimo-input='heater-2'][data-mimo-window='${key}']`) as HTMLInputElement).value;
      assert.strictEqual(field("baseline-start"), "");
      assert.strictEqual(field("baseline-end"), "");
      assert.strictEqual(field("settled-end"), "92");
    });
  });

  test("marks the window the engine's refusal names, and clears the mark on the next calculation", () => {
    withPage(({ document, presenter, posted }) => {
      presenter.tools([OFFER], undefined);
      presenter.setSelected(true);
      presenter.answer({ tool: "measuredPaths", inputs: {}, defaults: {}, failure: MISSING_BASELINE } as unknown as AnalysisEnvelope);
      const marked = Array.from(document.querySelectorAll("[data-mimo-window][aria-invalid='true']"), input => `${input.getAttribute("data-mimo-input")}:${input.getAttribute("data-mimo-window")}`);
      assert.deepStrictEqual(marked, ["heater-2:baseline-start"]);
      assert.strictEqual(document.querySelectorAll(".trust-input--error").length, 1, "the marked window is outlined as the other inputs' errors are");
      presenter.run();
      assert.strictEqual(document.querySelectorAll(".trust-input--error").length, 0);
      assert.strictEqual(document.querySelectorAll("[data-mimo-window][aria-invalid]").length, 0, "the next calculation clears the mark");
      assert.ok(posted.some(message => message.kind === ANALYSIS_KIND.run));
    });
  });
});
