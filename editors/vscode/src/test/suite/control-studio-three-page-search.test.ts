import * as assert from "assert";
import { formatEngineeringNumber } from "../../controlStudio/chartContract";
import { receiveEngineMessage, type EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import { tuneReceiver } from "../../controlStudio/webview/engineTune";
import { ThreePageTuning } from "../../controlStudio/webview/threePageTuning";
import { pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom");

/** The Tune page's controls and result area, as the tune bundle test builds them. */
const MARKUP = `<main>
  <select data-studio-controller><option value="PI">PI</option></select>
  <select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="manual">Manual</option></select>
  <div data-studio-tuning-result hidden></div>
</main>`;

const REASON = "The plan is refused: the window is too short.";

/**
 * Three trials of a search over the proportional gain: the first is met and selected, the second is attempted and not met, and the third
 * was refused before the search started, so it is not attempted.
 */
const TRIALS = [
  { parameters: { "Main.Kp": 1, "Main.Ki": 0.5 }, attempted: true, bounds: "met", score: 0.25 },
  { parameters: { "Main.Kp": 2, "Main.Ki": 0.5 }, attempted: true, bounds: "notMet", score: 0.1 },
  { parameters: { "Main.Kp": 3, "Main.Ki": 0.5 }, attempted: false, reason: REASON },
];

interface Page {
  readonly root: HTMLElement;
  readonly tuning: ThreePageTuning;
  readonly ports: EngineHostPorts;
  /** The payloads the page's flow stage received, as the operation router shows them. */
  readonly flows: Record<string, unknown>[];
}

/** Runs `check` on the Tune page, with the engine's messages routed as the stage router routes them. */
function withPage(check: (page: Page) => void): void {
  const dom = new JSDOM(MARKUP);
  const previous = (globalThis as any).document;
  (globalThis as any).document = dom.window.document;
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const loop = pidLoop();
    loop.signals.processVariable.unit = "rad";
    const tuning = new ThreePageTuning(root, loop, () => undefined, () => "PI");
    const flows: Record<string, unknown>[] = [];
    const ports: EngineHostPorts = {
      select: () => undefined,
      receive: (stage, payload) => {
        if (stage === "engineer") tuning.receive(payload);
        if (stage === "flow") flows.push(payload);
      },
    };
    check({ root, tuning, ports, flows });
  } finally {
    (globalThis as any).document = previous;
    dom.window.close();
  }
}

function trialRows(root: HTMLElement): HTMLTableRowElement[] {
  return Array.from(root.querySelectorAll<HTMLTableRowElement>("[data-studio-search-trial]"));
}

suite("Control Studio three-page finite search", () => {
  test("shows a finished search's trials: the selected one marked, a bound not met, a refused trial with its reason", () => {
    withPage(({ root, ports }) => {
      receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "tune", result: { kind: "searchAnswer", status: "completed", selected: 0, trials: TRIALS, scoreUnit: "rad²·s" } }, ports);
      const rows = trialRows(root);
      assert.deepStrictEqual(rows.map(row => row.dataset.studioSearchTrial), ["1", "2", "3"]);
      assert.ok(rows[0].hasAttribute("data-studio-search-selected"), "the selected trial is marked");
      assert.ok(!rows[1].hasAttribute("data-studio-search-selected"), "only the selected trial is marked");
      assert.match(rows[0].textContent ?? "", /selected/);
      assert.match(rows[0].textContent ?? "", /Met/);
      assert.ok((rows[0].textContent ?? "").includes(formatEngineeringNumber(0.25)), "J of the selected trial is shown");
      assert.match(rows[1].textContent ?? "", /Not met/);
      assert.match(rows[2].textContent ?? "", /Not attempted/);
      assert.strictEqual(rows[2].querySelector("[data-studio-search-reason]")?.textContent, REASON);
      const headings = Array.from(root.querySelectorAll("[data-studio-search-trials] th")).map(heading => heading.textContent);
      assert.ok(headings.includes("J (rad²·s)"), `J names the measurement's unit squared times seconds: ${headings.join(", ")}`);
      assert.ok(headings.some(heading => heading?.startsWith("Main.Kp")), "each coefficient has a column");
      assert.strictEqual(root.querySelector("[data-studio-search-status]"), null, "a completed search shows no sentence");
      assert.strictEqual((root.querySelector("[data-studio-tuning-result]") as HTMLElement).hidden, false);
    });
  });

  test("names J's unit as the engine answers it, not as the page builds it from the loop", () => {
    withPage(({ root, ports }) => {
      receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "tune", result: { kind: "searchAnswer", status: "completed", selected: 0, trials: TRIALS, scoreUnit: "N·m²·s" } }, ports);
      const headings = Array.from(root.querySelectorAll("[data-studio-search-trials] th")).map(heading => heading.textContent);
      assert.ok(headings.includes("J (N·m²·s)"), `the heading is the answer's unit: ${headings.join(", ")}`);
    });
  });

  test("shows the sentence of a search that did not complete, above its trials", () => {
    withPage(({ root, ports }) => {
      const message = "The search was cancelled; the trials not attempted stay marked.";
      receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "tune", result: { kind: "searchAnswer", status: "cancelled", message, trials: TRIALS, scoreUnit: "rad²·s" } }, ports);
      const sentence = root.querySelector<HTMLElement>("[data-studio-search-status]");
      assert.strictEqual(sentence?.dataset.studioSearchStatus, "cancelled");
      assert.strictEqual(sentence?.textContent, message);
      assert.strictEqual(root.querySelectorAll("[data-studio-search-selected]").length, 0, "a search with no selected trial marks none");
      assert.strictEqual(trialRows(root).length, 3, "the trials stay shown");
    });
  });

  test("reopens a kept search candidate's trials from the open answer", () => {
    withPage(({ root, ports }) => {
      assert.ok(tuneReceiver.opened, "the Tune stage shows an opened study");
      tuneReceiver.opened?.(root, { searchCandidate: { parameters: { "Main.Kp": 1, "Main.Ki": 0.5 }, selected: 0, trials: TRIALS, scoreUnit: "rad²·s" } }, ports);
      const rows = trialRows(root);
      assert.strictEqual(rows.length, 3, "every trial of the kept candidate is shown");
      assert.ok(rows[0].hasAttribute("data-studio-search-selected"), "the kept selected trial is marked");
      assert.strictEqual(root.querySelector("[data-studio-search-status]"), null, "a kept candidate has no sentence");
    });
  });

  test("shows the search's progress as the Tune flow's reason", () => {
    withPage(({ root, ports, flows }) => {
      receiveEngineMessage(root, { kind: "control-studio.engine.update", update: { command: "tune", state: "running", progress: "Trial 2 of 8.", operationId: "operation-1" } }, ports);
      assert.deepStrictEqual(flows.map(flow => [flow.status, flow.flowStage, flow.reason]), [["running", "Tune", "Trial 2 of 8."]]);
    });
  });

  test("a refused Tune drops the search the page showed", () => {
    withPage(({ root, tuning }) => {
      tuning.receive({ status: "search", search: { status: "completed", selected: 0, trials: TRIALS, scoreUnit: "rad²·s" } });
      assert.strictEqual(trialRows(root).length, 3);
      tuning.receive({ status: "failed", flowStage: "Tune", reason: "The runtime refused the search." });
      assert.strictEqual(trialRows(root).length, 0, "the refused search is no longer shown");
      assert.strictEqual((root.querySelector("[data-studio-tuning-result]") as HTMLElement).hidden, true);
    });
  });
});
