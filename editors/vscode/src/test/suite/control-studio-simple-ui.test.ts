import * as assert from "assert";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { IdentifiedModelPresentation } from "../../controlStudio/identification";
import { ThreePageOperation } from "../../controlStudio/webview/threePageOperation";
import { ThreePageAnalysisChooser } from "../../controlStudio/webview/threePageAnalysisChooser";
import { ThreePageTuning, MANUAL_PREVIEW_DELAY_MS } from "../../controlStudio/webview/threePageTuning";
import { renderModelPresentation } from "../../controlStudio/webview/threePageModel";
import { ThreePageLiveSignals } from "../../controlStudio/webview/threePageLiveSignals";
import { parseDurationSeconds, formatDurationSeconds } from "../../controlStudio/webview/durationField";
import { plainReason, studioFailureOf } from "../../controlStudio/webview/studioFailure";
import { signalRoleLabel, signalRoleTitle } from "../../controlStudio/webview/signalRoles";
import { renderValidationResults } from "../../controlStudio/webview/threePageResults";
import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { openCompactNumberEditor } from "../../webview/compactEditor";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import { THREE_PAGE_STYLES } from "../../controlStudio/threePageStyles";
import type { ControlStudioView } from "../../controlStudio/panelTemplate";
import { open } from "./control-studio-loop-setup-form-harness";

const { JSDOM } = require("jsdom");

/** Installs a DOM as the global document for the webview modules, and restores the previous globals afterwards. */
function install(document: string): { window: any; document: Document; restore: () => void } {
  const window = new JSDOM(document).window;
  const names = ["document", "window", "HTMLElement", "HTMLButtonElement", "HTMLSelectElement", "HTMLInputElement", "Node"];
  const saved = names.map(name => [name, (globalThis as any)[name]] as const);
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, HTMLButtonElement: window.HTMLButtonElement,
    HTMLSelectElement: window.HTMLSelectElement, HTMLInputElement: window.HTMLInputElement, Node: window.Node });
  // JSDOM has no modal dialog; a dialog opens with its open attribute, as in the shipped webview.
  const proto = window.HTMLElement.prototype as any;
  if (typeof proto.showModal !== "function") proto.showModal = function (this: HTMLElement) { this.setAttribute("open", ""); };
  if (typeof proto.close !== "function") proto.close = function (this: HTMLElement) { this.removeAttribute("open"); this.dispatchEvent(new window.Event("close")); };
  if (!(globalThis as any).crypto?.randomUUID) (globalThis as any).crypto = { randomUUID: () => require("crypto").randomUUID() };
  return { window, document: window.document, restore: () => {
    for (const [name, value] of saved) { if (value === undefined) delete (globalThis as any)[name]; else (globalThis as any)[name] = value; }
    window.close();
  } };
}

function fragment(html: string): { window: any; document: Document; restore: () => void } {
  return install(`<!doctype html><html><body>${html}</body></html>`);
}

/** The three pages' action areas and the controls the operation drives. */
const OPERATION_DOM = `<main data-studio-three-page>
 <section data-studio-page="model">
  <button data-engineer-find>Identify plant</button><button data-studio-limits>Limits…</button><button data-studio-requirements>Requirements…</button>
  <select data-engineer-acquisition-method><option value="automatic">Automatic</option></select>
  <p class="studio-action-message" data-studio-message="model" role="status" hidden></p>
  <p class="studio-failure-detail" id="studio-failure-detail-model" data-studio-failure-detail="model" hidden></p>
  <button type="button" class="trust-button studio-failure-action" data-studio-failure-action="model" hidden></button>
  <input type="file" data-identify-import hidden>
 </section>
 <section data-studio-page="tune">
  <select data-engineer-tuning-method><option value="automatic">Automatic</option></select>
  <button data-engineer-tune>Tune controller</button>
  <p class="studio-action-message" data-studio-message="tune" role="status" hidden></p>
  <p class="studio-failure-detail" id="studio-failure-detail-tune" data-studio-failure-detail="tune" hidden></p>
  <button type="button" class="trust-button studio-failure-action" data-studio-failure-action="tune" hidden></button>
 </section>
 <section data-studio-page="verify">
  <button data-engineer-verify>Validate controller</button>
  <p class="studio-action-message" data-studio-message="verify" role="status" hidden></p>
  <p class="studio-failure-detail" id="studio-failure-detail-verify" data-studio-failure-detail="verify" hidden></p>
  <button type="button" class="trust-button studio-failure-action" data-studio-failure-action="verify" hidden></button>
  <div data-studio-application hidden><button data-engineer-apply disabled>Apply</button></div>
 </section>
</main>`;

function operationRoot(): { root: HTMLElement; restore: () => void; document: Document; window: any } {
  const dom = fragment(OPERATION_DOM);
  return { root: dom.document.querySelector<HTMLElement>("[data-studio-three-page]")!, restore: dom.restore, document: dom.document, window: dom.window };
}

const noop = () => undefined;

suite("Control Studio simple by default", () => {
  test("a failure shows one sentence, one fix and the technical reason in place", () => {
    const dom = operationRoot();
    try {
      const actions: string[] = [];
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => undefined, (kind, page) => actions.push(`${page}:${kind}`));
      operation.failure("model", { message: "The output range is too small.", detail: "Output 0–100 %, the test needs 120 %.",
        nextAction: { kind: "open-limits", label: "Limits…" } });
      const area = dom.root.querySelector<HTMLElement>('[data-studio-message="model"]')!;
      const sentence = area.querySelector<HTMLButtonElement>(".studio-failure-sentence")!;
      const detail = dom.root.querySelector<HTMLElement>('[data-studio-failure-detail="model"]')!;
      const action = dom.root.querySelector<HTMLButtonElement>('[data-studio-failure-action="model"]')!;
      assert.strictEqual(area.textContent, "The output range is too small.");
      assert.strictEqual(detail.hidden, true, "the technical reason starts hidden");
      assert.strictEqual(action.hidden, false);
      assert.strictEqual(action.textContent, "Limits…");
      sentence.click();
      assert.strictEqual(detail.hidden, false, "selecting the sentence reveals the reason in place");
      assert.strictEqual(detail.textContent, "Output 0–100 %, the test needs 120 %.");
      assert.strictEqual(sentence.getAttribute("aria-expanded"), "true");
      action.click();
      assert.deepStrictEqual(actions, ["model:open-limits"]);
    } finally { dom.restore(); }
  });

  test("only the fixes that open or start something show a button", () => {
    const dom = operationRoot();
    try {
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => undefined, noop);
      const action = dom.root.querySelector<HTMLButtonElement>('[data-studio-failure-action="tune"]')!;
      const cases: [string, boolean][] = [["open-limits", true], ["open-requirements", true], ["open-channels", true], ["choose-method", true],
        ["identify", true], ["tune", true], ["validate", true], ["restore", true], ["connect-runtime", true], ["open-loop-setup", true],
        ["approve", false], ["wait", false], ["none", false]];
      for (const [kind, shown] of cases) {
        operation.failure("tune", { message: `Reason for ${kind}.`, nextAction: { kind, label: "Fix" } });
        assert.strictEqual(action.hidden, !shown, kind);
      }
      operation.failure("tune", { message: "A fix without a label.", nextAction: { kind: "tune", label: "" } });
      assert.strictEqual(action.hidden, true, "an empty label shows no button");
    } finally { dom.restore(); }
  });

  test("a reason with a Next sentence shows only its message", () => {
    const dom = operationRoot();
    try {
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => undefined, noop);
      operation.notice("model", "Enter the output range. Next: Open Limits….");
      assert.strictEqual(dom.root.querySelector<HTMLElement>('[data-studio-message="model"]')!.textContent, "Enter the output range.");
      assert.strictEqual(dom.root.querySelector<HTMLButtonElement>('[data-studio-failure-action="model"]')!.hidden, true);
      assert.strictEqual(plainReason("Plain sentence."), "Plain sentence.");
      assert.deepStrictEqual(studioFailureOf({ reason: "Enter the output range. Next: Open Limits…." }),
        { message: "Enter the output range.", nextAction: { kind: "none", label: "" } });
      assert.strictEqual(studioFailureOf({ failure: { nextAction: "reapply" }, reason: "Apply cancelled." })?.message, "Apply cancelled.");
    } finally { dom.restore(); }
  });

  test("an empty plot names what it waits for", () => {
    const live = new ThreePageLiveSignals({} as ControlLoopDefinition);
    assert.strictEqual(live.emptyMessage(), "Start the runtime to see live values", "no monitor sample yet");
    live.receive({ availability: "unavailable", source: "runtime", loopId: "room", reason: "The runtime is stopped." } as any);
    assert.strictEqual(live.emptyMessage(), "Start the runtime to see live values");
    live.receive({ availability: "available", runtimeSessionId: "s1", scan: 1, observedAtMs: 0 } as any);
    assert.strictEqual(live.emptyMessage(), "Waiting for the first samples");
  });

  test("role names carry the full symbol as their hover text", () => {
    const loop = { id: "room", signals: {
      processVariable: { symbol: "P1.RoomC", unit: "degC", id: "processVariable" },
      manipulatedVariable: { symbol: "P1.Heater", unit: "%", id: "manipulatedVariable" },
      setpoint: { symbol: "P1.SetpointC", unit: "degC" }, additional: [] } } as unknown as ControlLoopDefinition;
    assert.strictEqual(signalRoleLabel(loop, "P1.RoomC", "degC"), "Measurement (°C)");
    assert.strictEqual(signalRoleLabel(loop, "P1.Heater", "%"), "Output (%)");
    assert.strictEqual(signalRoleTitle(loop), "Measurement: P1.RoomC · Output: P1.Heater · Setpoint: P1.SetpointC");
    const dom = fragment("<main></main>");
    try {
      const dialog = openCompactNumberEditor({ title: "Control requirements", fieldAttribute: "data-studio-requirement", rowLayout: "requirements",
        showSignals: true, fields: [{ key: "maximumError", label: "Maximum steady-state error (°C)", signal: "P1.RoomC", signalLabel: "Measurement (°C)", value: 0.2, minimum: 0 }],
        commit: noop });
      const signal = dialog.querySelector<HTMLElement>('span[title="P1.RoomC"]')!;
      assert.strictEqual(signal.textContent, "Measurement (°C)");
    } finally { dom.restore(); }
  });

  test("durations parse and show in their largest fitting unit", () => {
    assert.strictEqual(parseDurationSeconds("90 s"), 90);
    assert.strictEqual(parseDurationSeconds("15 min"), 900);
    assert.strictEqual(parseDurationSeconds("2 h"), 7200);
    assert.strictEqual(parseDurationSeconds("1 h 30 min"), 5400);
    assert.strictEqual(parseDurationSeconds("45"), 45);
    assert.strictEqual(parseDurationSeconds(" 2 min 5 s "), 125);
    assert.strictEqual(parseDurationSeconds("12 m"), undefined, "a bare m is not a duration unit");
    assert.strictEqual(parseDurationSeconds("soon"), undefined);
    assert.strictEqual(parseDurationSeconds(""), undefined);
    assert.strictEqual(formatDurationSeconds(90), "90 s");
    assert.strictEqual(formatDurationSeconds(900), "15 min");
    assert.strictEqual(formatDurationSeconds(125), "2 min 5 s");
    assert.strictEqual(formatDurationSeconds(4500), "75 min");
    // Below 2 h the largest fitting unit is minutes (specification 35, B9.9).
    assert.strictEqual(formatDurationSeconds(5400), "90 min");
    assert.strictEqual(formatDurationSeconds(7200), "2 h");
    for (const seconds of [0.5, 45, 125, 4500, 5400, 7325]) assert.strictEqual(parseDurationSeconds(formatDurationSeconds(seconds)), seconds, `${seconds} s`);
  });

  test("the model shows a key-numbers line, a fraction and its warnings", () => {
    const dom = fragment(`<main data-studio-three-page><div data-studio-model-result hidden><p data-studio-model-type></p>
      <p class="studio-model-equation" data-studio-model-equation></p><p class="studio-model-numbers" data-studio-model-numbers hidden></p>
      <ul class="studio-model-warnings" data-studio-model-warnings hidden></ul></div></main>`);
    try {
      const root = dom.document.querySelector<HTMLElement>("[data-studio-three-page]")!;
      const presentation = {
        equation: "G(s) = 0.21 · e^(−12s) / (4500s + 1)", units: { input: "%", output: "°C", gain: "°C/%", time: "s" },
        parameters: [{ name: "Gain", value: 0.21, unit: "°C/%" }, { name: "Time constant 1", value: 4500, unit: "s" }, { name: "Dead time", value: 12, unit: "s" }],
        poles: [-1 / 4500], zeros: [], delaySeconds: 12, warnings: ["The response reached 87 % of its fitted final change."],
      } as unknown as IdentifiedModelPresentation;
      renderModelPresentation(root, presentation, "First-order model with delay");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-numbers]")!.textContent, "Gain 0.21 °C per %, time constant 75 min, delay 12 s");
      assert.strictEqual(root.querySelectorAll("[data-studio-model-warnings] li").length, 1);
      assert.strictEqual(root.querySelector("[data-studio-model-warnings] li")!.textContent, "The response reached 87 % of its fitted final change.");
      assert.ok(root.querySelector(".studio-math-fraction .studio-math-num"), "the equation is a stacked fraction");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden, false);
    } finally { dom.restore(); }
  });

  test("a manual edit posts one preview after the pause", async () => {
    const posts: { kind: string; payload?: Record<string, unknown> }[] = [];
    const tuning: any = Object.create(ThreePageTuning.prototype);
    tuning.post = (kind: string, payload?: Record<string, unknown>) => { posts.push({ kind, payload }); };
    tuning.previewLater({ Kp: 1 });
    tuning.previewLater({ Kp: 2 });
    tuning.previewLater({ Kp: 3 });
    assert.strictEqual(posts.length, 0, "nothing is posted before the pause");
    assert.strictEqual(MANUAL_PREVIEW_DELAY_MS, 300);
    await new Promise(resolve => setTimeout(resolve, MANUAL_PREVIEW_DELAY_MS + 150));
    assert.deepStrictEqual(posts, [{ kind: "control-studio.engineer.manual-preview", payload: { settings: { Kp: 3 } } }]);
  });

  test("Analyze… lists the tools, a choice opens its tool, and Close returns the tuning result", () => {
    const dom = fragment(`<main><button data-studio-analyze hidden aria-expanded="false">Analyze…</button><div data-studio-analyze-list hidden></div>
      <button data-studio-analysis-close hidden>Close</button></main>`);
    try {
      const root = dom.document.querySelector<HTMLElement>("main")!;
      const chosen: string[] = [];
      let closed = 0;
      // The tool ids are the engine's (B9.9, `controlStudio/analyze`): the chooser lists and labels them by these ids.
      const tools = ["rootLocus", "frequency"];
      const chooser = new ThreePageAnalysisChooser(root, { labels: { rootLocus: "Root locus", frequency: "Bode (frequency response)" },
        tools: () => tools, choose: tool => { chosen.push(tool); }, close: () => { closed += 1; } });
      const analyze = root.querySelector<HTMLButtonElement>("[data-studio-analyze]")!;
      const list = root.querySelector<HTMLElement>("[data-studio-analyze-list]")!;
      const close = root.querySelector<HTMLButtonElement>("[data-studio-analysis-close]")!;
      chooser.synchronize(false);
      assert.strictEqual(analyze.hidden, false, "Analyze… shows once a tool exists");
      assert.strictEqual(close.hidden, true);
      analyze.click();
      const choices = Array.from(list.querySelectorAll<HTMLButtonElement>("button"));
      assert.deepStrictEqual(choices.map(choice => choice.textContent), ["Root locus", "Bode (frequency response)"]);
      choices[1].click();
      assert.deepStrictEqual(chosen, ["frequency"]);
      assert.strictEqual(list.hidden, true, "choosing a tool closes the list");
      chooser.synchronize(true);
      assert.strictEqual(close.hidden, false, "Close shows while a tool is open");
      close.click();
      assert.strictEqual(closed, 1);
      tools.length = 0;
      chooser.synchronize(false);
      assert.strictEqual(analyze.hidden, true, "no tool, no Analyze…");
    } finally { dom.restore(); }
  });

  test("the Tuning method select holds tuning methods only", () => {
    const html = renderThreePageStudioHtml({ cspSource: "x", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", project: "p",
      session: "s", loop: "l", loopRevision: "r", revision: "v", processUnit: "PV", outputUnit: "MV" } as unknown as ControlStudioView, text => text);
    const dom = install(html);
    try {
      const values = Array.from(dom.document.querySelectorAll<HTMLOptionElement>("[data-engineer-tuning-method] option")).map(option => option.value);
      assert.deepStrictEqual(values, ["automatic", "SIMC", "AMIGO", "relay", "manual"]);
      assert.strictEqual(dom.document.querySelector<HTMLButtonElement>("[data-studio-analyze]")!.hidden, true);
    } finally { dom.restore(); }
  });

  test("Apply sits under the results table above the plot, and each verdict has an icon", () => {
    const html = renderThreePageStudioHtml({ cspSource: "x", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", project: "p",
      session: "s", loop: "l", loopRevision: "r", revision: "v", processUnit: "PV", outputUnit: "MV" } as unknown as ControlStudioView, text => text);
    const dom = install(html);
    try {
      const apply = dom.document.querySelector<HTMLElement>("[data-studio-application]")!;
      const table = dom.document.querySelector<HTMLElement>("[data-studio-validation-result]")!;
      const plot = dom.document.querySelector<HTMLElement>(".studio-response")!;
      const following = 4; // Node.DOCUMENT_POSITION_FOLLOWING
      assert.ok(dom.document.getElementById("studio-page-verify")!.contains(apply));
      assert.ok(table.compareDocumentPosition(apply) & following, "Apply follows the results table");
      assert.ok(apply.compareDocumentPosition(plot) & following, "Apply precedes the plot");
      for (const verdict of ["pass", "fail", "inconclusive"]) assert.ok(THREE_PAGE_STYLES.includes(`data-verdict="${verdict}"`), verdict);
    } finally { dom.restore(); }
  });

  test("a result row keeps its word and marks its verdict", () => {
    const dom = fragment(`<main data-studio-three-page><table data-studio-validation-result hidden><thead></thead><tbody></tbody></table></main>`);
    try {
      const root = dom.document.querySelector<HTMLElement>("[data-studio-three-page]")!;
      renderValidationResults(root, { status: "verified", candidate: {
        requirements: [{ id: "settling", kind: "settling", status: "Fail", value: 5, limit: 4, unit: "s", reason: "late" },
          { id: "error", kind: "error", status: "Pass", value: 0.1, limit: 0.2, unit: "°C" }],
        specification: [{ id: "settling", kind: "settling" }, { id: "error", kind: "error" }] } } as any);
      const cells = Array.from(root.querySelectorAll<HTMLElement>("tbody tr")).map(row => row.lastElementChild as HTMLElement);
      assert.deepStrictEqual(cells.map(cell => [cell.textContent, cell.dataset.verdict]), [["Fail · late", "fail"], ["Pass", "pass"]]);
    } finally { dom.restore(); }
  });

  test("the requirements editor shows the working rows and keeps the test design under More…", () => {
    const dom = fragment("<main></main>");
    try {
      const loop = {
        id: "room", name: "Room", controller: { instance: "P1.Controller", type: "Control.PID" }, limits: { outputMinimum: 0, outputMaximum: 100 },
        signals: {
          processVariable: { symbol: "P1.RoomC", unit: "degC", range: { minimum: 0, maximum: 40 }, id: "processVariable" },
          manipulatedVariable: { symbol: "P1.Heater", unit: "%", range: { minimum: 0, maximum: 100 }, id: "manipulatedVariable" },
          setpoint: { symbol: "P1.SetpointC", unit: "degC", range: { minimum: 0, maximum: 40 }, id: "setpoint" }, additional: [],
        },
      } as unknown as ControlLoopDefinition;
      editControlRequirements(loop, { referenceChange: 5, maximumError: 0.2, settlingBand: 0.5, settlingTimeSeconds: 900, durationSeconds: 3600 }, noop);
      const dialog = dom.document.querySelector<HTMLElement>("[data-trust-compact-editor]")!;
      const visible = Array.from(dialog.querySelectorAll<HTMLElement>("[data-studio-requirement]"))
        .filter(input => !input.closest(".trust-compact-editor-more-body")).map(input => input.dataset.studioRequirement);
      assert.deepStrictEqual(visible, ["settlingTimeSeconds", "settlingBand", "maximumOvershootPercent", "maximumError", "constantInputDisturbance"]);
      const more = dialog.querySelector<HTMLDetailsElement>("[data-trust-compact-more]")!;
      assert.strictEqual(more.open, false, "More… starts collapsed");
      assert.deepStrictEqual(Array.from(more.querySelectorAll<HTMLElement>("[data-studio-requirement]")).map(input => input.dataset.studioRequirement),
        ["referenceChange", "durationSeconds", "transferTolerance"]);
      assert.ok(dialog.querySelector(".trust-compact-editor-no-signal"), "a loop with one output has no Signal column");
    } finally { dom.restore(); }
  });

  test("Done marks every invalid field at once, and a duration keeps its own text", () => {
    const dom = fragment("<main></main>");
    try {
      let committed: Record<string, number> | undefined;
      const dialog = openCompactNumberEditor({ title: "Experiment limits", fieldAttribute: "data-studio-limit", fields: [
        { key: "inputMinimum", label: "Minimum" }, { key: "outputMaximum", label: "Maximum" },
        { key: "durationSeconds", label: "Maximum duration", value: 900, duration: true, minimum: 1, maximum: 7200 }],
        commit: values => { committed = { ...values }; } });
      dialog.querySelector("form")!.dispatchEvent(new dom.window.Event("submit", { bubbles: true, cancelable: true }));
      const notices = Array.from(dialog.querySelectorAll<HTMLElement>("[role=alert]")).filter(notice => !notice.hidden).map(notice => notice.textContent);
      assert.deepStrictEqual(notices, ["Enter minimum.", "Enter maximum."]);
      assert.strictEqual(committed, undefined);
      let seconds: Record<string, number> | undefined;
      const limit = openCompactNumberEditor({ title: "Experiment limits", fieldAttribute: "data-studio-limit", fields: [
        { key: "durationSeconds", label: "Maximum duration", value: 900, duration: true, minimum: 1, maximum: 7200 }],
        commit: values => { seconds = { ...values }; } });
      const text = limit.querySelector<HTMLInputElement>("input")!;
      assert.strictEqual(text.value, "15 min");
      text.value = "2 min";
      limit.querySelector("form")!.dispatchEvent(new dom.window.Event("submit", { bubbles: true, cancelable: true }));
      assert.deepStrictEqual(seconds, { durationSeconds: 120 });
    } finally { dom.restore(); }
  });

  test("the setup summary states what the code provides, and Change reveals its selector", () => {
    const model = {
      structure: "pid", addressedReal: ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"], booleans: ["P1.AutoMode"],
      records: ["P1.Parameters"], writes: [], initializes: [], pids: [{ instance: "P1.Controller", task: { name: "MainTask", intervalMs: 100 } }],
      instances: ["P1"], reals: ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"],
      prefill: { instance: "P1.Controller", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.AppliedPowerPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualOutput" },
    };
    const form = open(model);
    try {
      assert.strictEqual(form.root.querySelector<HTMLElement>('[data-setup-found-value="manipulatedVariable"]')!.textContent, "P1.AppliedPowerPct");
      assert.strictEqual(form.root.querySelector<HTMLElement>('[data-setup-found-value="task"]')!.textContent, "MainTask (100 ms)");
      assert.strictEqual(form.root.querySelector<HTMLInputElement>('[data-setup-field="name"]')!.value, "Controller", "the loop name follows the controller");
      const panel = form.root.querySelector<HTMLElement>('[data-setup-change-panel="processVariable"]')!;
      const change = form.root.querySelector<HTMLButtonElement>('[data-setup-change="processVariable"]')!;
      assert.strictEqual(panel.hidden, true);
      change.click();
      assert.strictEqual(panel.hidden, false, "Change reveals the measurement selector");
      assert.strictEqual(change.getAttribute("aria-expanded"), "true");
      assert.strictEqual(form.root.querySelector('[data-setup-field="experimentStep"]'), null, "no identification step is asked");
      assert.strictEqual(form.root.querySelector('[data-setup-field="experimentDuration"]'), null, "no identification duration is asked");
    } finally { form.close(); }
  });

  test("Tune and the import wait for their gates, not for a model alone", () => {
    const dom = operationRoot();
    try {
      let gate: string | undefined = "The plant mapping is incomplete.";
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => gate, noop);
      operation.modelArrived();
      const tune = dom.root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!;
      const importControl = dom.root.querySelector<HTMLInputElement>("[data-identify-import]")!;
      assert.strictEqual(tune.disabled, true, "a model does not override a closed gate");
      operation.synchronize();
      assert.strictEqual(importControl.disabled, true, "the idle lock keeps the import closed while its gate is closed");
      gate = undefined;
      operation.synchronize();
      assert.strictEqual(tune.disabled, false, "Tune opens once the model and every gate agree");
      assert.strictEqual(importControl.disabled, false);
    } finally { dom.restore(); }
  });
});
