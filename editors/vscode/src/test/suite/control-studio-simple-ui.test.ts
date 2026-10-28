import * as assert from "assert";
import type { LiveSides, LoopDefinition } from "../../controlStudio/generated/OpenResult";
import type { IdentifiedModelPresentation } from "../../controlStudio/generated/ManualIdentificationAnswer";
import { ThreePageOperation } from "../../controlStudio/webview/threePageOperation";
import { applyStageStatuses } from "../../controlStudio/webview/engineShared";
import { ThreePageAnalysisChooser } from "../../controlStudio/webview/threePageAnalysisChooser";
import { ThreePageTuning, MANUAL_PREVIEW_DELAY_MS } from "../../controlStudio/webview/threePageTuning";
import { renderModelPresentation } from "../../controlStudio/webview/threePageModel";
import { ThreePageLiveSignals } from "../../controlStudio/webview/threePageLiveSignals";
import { formatDurationSeconds } from "../../controlStudio/webview/durationField";
import { nextActionHasButton } from "../../controlStudio/webview/studioFailure";
import { studioFailureOf } from "../../controlStudio/webview/studioFailure";
import { signalRoleLabel, signalRoleName, signalRoleTitle } from "../../controlStudio/webview/signalRoles";
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

  test("a failure shows a button exactly when the engine names its words", () => {
    const dom = operationRoot();
    try {
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => undefined, noop);
      const action = dom.root.querySelector<HTMLButtonElement>('[data-studio-failure-action="tune"]')!;
      // The labels are the engine's `nextActionLabel` (NextAction::label); a wait or no action names none (specification 35, A11.2).
      const cases: [string, string, boolean][] = [["open-limits", "Limits…", true], ["open-requirements", "Requirements…", true],
        ["open-channels", "Channels…", true], ["choose-method", "Choose another method", true], ["identify", "Identify plant", true],
        ["tune", "Tune controller", true], ["validate", "Validate controller", true], ["restore", "Restore previous settings", true],
        ["connect-runtime", "Connect runtime…", true], ["open-loop-setup", "Loop setup…", true], ["approve", "Approve…", true],
        ["wait", "", false], ["none", "", false]];
      for (const [kind, label, shown] of cases) {
        operation.failure("tune", { message: `Reason for ${kind}.`, nextAction: { kind, label } });
        assert.strictEqual(action.hidden, !shown, kind);
        if (shown) assert.strictEqual(action.textContent, label, kind);
      }
      operation.failure("tune", { message: "A fix without a label.", nextAction: { kind: "tune", label: "" } });
      assert.strictEqual(action.hidden, true, "an empty label shows no button");
    } finally { dom.restore(); }
  });

  test("a refused Channels… selection stays in the Tune action area with its sentence and the Channels… action, and no other failure moves", () => {
    const dom = operationRoot();
    try {
      const actions: string[] = [];
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => undefined, (kind, page) => actions.push(`${page}:${kind}`));
      const sentence = "Choose a different signal; this signal is already assigned to another channel.";
      const area = (page: string) => dom.root.querySelector<HTMLElement>(`[data-studio-message="${page}"]`)!;
      const action = (page: string) => dom.root.querySelector<HTMLButtonElement>(`[data-studio-failure-action="${page}"]`)!;
      // The answer to a change of the study inputs names the page that owns the fix (`page`, engineOpened.ts); the operation shown last is Identify's by default.
      operation.receive("flow", { status: "failed", page: "tune", failure: { message: sentence, nextAction: { kind: "open-channels", label: "Channels…" } } });
      // Wrong behaviour caught: the sentence landing in the Identify area, which the engineer on the Tune page, where Channels… is, never sees.
      assert.strictEqual(area("tune").textContent, sentence);
      assert.strictEqual(area("tune").hidden, false);
      assert.strictEqual(action("tune").textContent, "Channels…");
      assert.strictEqual(action("tune").hidden, false);
      assert.strictEqual(area("model").textContent, "", "Identify's area shows nothing of it");
      assert.strictEqual(area("model").hidden, true);
      assert.strictEqual(action("model").hidden, true);
      action("tune").click();
      assert.deepStrictEqual(actions, ["tune:open-channels"], "the action opens the Channels… editor from the page that shows it");
      // A failure that names no page shows on the page of the operation, as before; an unknown page name is no page.
      operation.receive("flow", { status: "failed", failure: { message: "The runtime refused.", nextAction: { kind: "connect-runtime", label: "Connect runtime…" } } });
      assert.strictEqual(area("model").textContent, "The runtime refused.");
      operation.receive("flow", { status: "failed", page: "elsewhere", failure: { message: "Another refusal.", nextAction: { kind: "none", label: "" } } });
      assert.strictEqual(area("model").textContent, "Another refusal.");
    } finally { dom.restore(); }
  });

  test("a paused or cancelled flow shows the engine's sentence, and writes none of its own", () => {
    const dom = operationRoot();
    try {
      const operation = new ThreePageOperation(dom.root, noop, noop, noop);
      const area = dom.root.querySelector<HTMLElement>('[data-studio-message="tune"]')!;
      operation.receive("flow", { status: "running", flowStage: "Tune", reason: "Designing the controller" });
      assert.strictEqual(area.textContent, "Designing the controller");
      // The cancelled update's `message` reaches the flow as its reason (engineOpened.ts): the page shows those words.
      operation.receive("flow", { status: "cancelled", flowStage: "Tune", reason: "The operation was cancelled." });
      assert.strictEqual(area.textContent, "The operation was cancelled.");
      operation.receive("flow", { status: "paused", flowStage: "Tune", reason: "Enter the requirements." });
      assert.strictEqual(area.textContent, "Enter the requirements.");
      // Wrong behaviour caught: an invented "Operation stopped." where the engine sent no sentence.
      operation.receive("flow", { status: "cancelled", flowStage: "Tune" });
      assert.strictEqual(area.textContent, "");
      assert.strictEqual(area.hidden, true);
    } finally { dom.restore(); }
  });

  test("a reason is shown as the engine worded it, and a reason alone names no fix", () => {
    const dom = operationRoot();
    try {
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => undefined, noop);
      operation.notice("model", "Enter the output range.");
      assert.strictEqual(dom.root.querySelector<HTMLElement>('[data-studio-message="model"]')!.textContent, "Enter the output range.");
      assert.strictEqual(dom.root.querySelector<HTMLButtonElement>('[data-studio-failure-action="model"]')!.hidden, true);
      assert.deepStrictEqual(studioFailureOf({ reason: "Enter the output range." }),
        { message: "Enter the output range.", nextAction: { kind: "none", label: "" } });
      assert.strictEqual(studioFailureOf({ failure: { nextAction: "reapply" }, reason: "Apply cancelled." })?.message, "Apply cancelled.");
    } finally { dom.restore(); }
  });

  test("an empty plot names what it waits for", () => {
    const live = new ThreePageLiveSignals();
    assert.strictEqual(live.emptyMessage(), "Start the runtime to see live values", "no monitor sample yet");
    live.receive({ availability: "unavailable", source: "runtime", loopId: "room", reason: "The runtime is stopped." } as any);
    assert.strictEqual(live.emptyMessage(), "Start the runtime to see live values");
    live.receive({ availability: "available", runtimeSessionId: "s1", scan: 1, observedAtMs: 0 } as any);
    assert.strictEqual(live.emptyMessage(), "Waiting for the first samples");
  });

  test("role names carry the full symbol as their hover text", () => {
    // The engine's open answer (`liveSides`, B3.6, B9.6) for a loop with one channel on each side, a declared setpoint and an actuator: the
    // applied output (`mv`) is the actuator's position and the requested output (`requestedMv`) is the manipulated variable.
    const sides: LiveSides = {
      inputs: [{ signalId: "manipulatedVariable", symbol: "P1.Heater", unit: "%" }],
      outputs: [{ signalId: "processVariable", symbol: "P1.RoomC", unit: "degC" }],
      plotSignals: [{ id: "sp", kind: "setpoint", symbol: "P1.SetpointC", unit: "degC", span: 100 },
        { id: "pv", kind: "measurement", symbol: "P1.RoomC", unit: "degC", span: 100 },
        { id: "mv", kind: "appliedOutput", symbol: "P1.HeaterPosition", unit: "%", span: 100 },
        { id: "requestedMv", kind: "requestedOutput", symbol: "P1.Heater", unit: "%", span: 100 }],
    };
    assert.strictEqual(signalRoleLabel(sides, "P1.RoomC", "degC"), "Measurement (°C)");
    assert.strictEqual(signalRoleLabel(sides, "P1.Heater", "%"), "Output (%)");
    assert.strictEqual(signalRoleLabel(sides, "P1.SetpointC", "degC"), "Setpoint (°C)");
    assert.strictEqual(signalRoleLabel(sides, "P1.Pressure", "1"), "P1.Pressure", "a signal without a role is named by its symbol, and a dimensionless unit adds none");
    // The actuator's position is no role of its own: a title that listed it would name a signal the legend never calls by a role.
    assert.strictEqual(signalRoleTitle(sides), "Measurement: P1.RoomC · Output: P1.Heater · Setpoint: P1.SetpointC");
    assert.strictEqual(signalRoleName(undefined, "P1.RoomC"), undefined, "before the engine's open answer no signal has a role");
    assert.strictEqual(signalRoleTitle(null), "");
    const dom = fragment("<main></main>");
    try {
      const dialog = openCompactNumberEditor({ title: "Control requirements", fieldAttribute: "data-studio-requirement", rowLayout: "requirements",
        showSignals: true, fields: [{ key: "maximumError", label: "Maximum steady-state error (°C)", signal: "P1.RoomC", signalLabel: "Measurement (°C)", value: 0.2 }],
        commit: noop });
      const signal = dialog.querySelector<HTMLElement>('span[title="P1.RoomC"]')!;
      assert.strictEqual(signal.textContent, "Measurement (°C)");
    } finally { dom.restore(); }
  });

  test("channels of one kind are numbered in the engine's channel order and an additional signal is named by its identifier", () => {
    // Two measured outputs and two plant inputs, as `liveSides` lists them in channel order; the plot list is in the order of the Signals
    // selector (sp, pv, mv, then the additional signals), which is not the channel order.
    const sides: LiveSides = {
      inputs: [{ signalId: "manipulatedVariable", symbol: "P1.Valve", unit: "%" }, { signalId: "pump", symbol: "P1.Pump", unit: "%" }],
      outputs: [{ signalId: "pressure", symbol: "P1.Pressure", unit: "bar" }, { signalId: "processVariable", symbol: "P1.Level", unit: "m" }],
      plotSignals: [{ id: "pv", kind: "measurement", symbol: "P1.Level", unit: "m", span: 10 },
        { id: "mv", kind: "appliedOutput", symbol: "P1.Valve", unit: "%", span: 100 },
        { id: "pressure", kind: "additional", symbol: "P1.Pressure", unit: "bar", span: 20 },
        { id: "pump", kind: "additional", symbol: "P1.Pump", unit: "%", span: 100 },
        { id: "inflow", kind: "additional", symbol: "P1.Inflow", unit: "m3/h", span: 50 }],
    };
    // Wrong behaviour caught: numbering by the position in the plot list would call the pressure "Measurement 2" and the level "Measurement 1".
    assert.strictEqual(signalRoleName(sides, "P1.Pressure"), "Measurement 1");
    assert.strictEqual(signalRoleName(sides, "P1.Level"), "Measurement 2");
    assert.strictEqual(signalRoleName(sides, "P1.Valve"), "Output 1");
    assert.strictEqual(signalRoleName(sides, "P1.Pump"), "Output 2");
    // An additional signal that no channel binds is named by its identifier; a symbol the engine does not list has no role.
    assert.strictEqual(signalRoleName(sides, "P1.Inflow"), "inflow");
    assert.strictEqual(signalRoleName(sides, "P1.Unlisted"), undefined);
    assert.strictEqual(signalRoleTitle(sides),
      "Measurement 1: P1.Pressure · Measurement 2: P1.Level · Output 1: P1.Valve · Output 2: P1.Pump · inflow: P1.Inflow");
  });

  test("the output the controller requested is named by its identifier when no channel carries it, as an additional signal is", () => {
    // The loop's input channel binds the actuator's position, so the requested output (the manipulated variable) is no channel: the engine lists it with
    // its own kind, and the name follows the kind. A name that fixed `requestedMv` among the identifiers of the setpoint, the measurement and the
    // applied output answered nothing here, while the plot coloured the same signal as an auxiliary one.
    const sides: LiveSides = {
      inputs: [{ signalId: "actuator", symbol: "P1.HeaterPosition", unit: "%" }],
      outputs: [{ signalId: "processVariable", symbol: "P1.RoomC", unit: "degC" }],
      plotSignals: [{ id: "pv", kind: "measurement", symbol: "P1.RoomC", unit: "degC", span: 100 },
        { id: "mv", kind: "appliedOutput", symbol: "P1.HeaterPosition", unit: "%", span: 100 },
        { id: "requestedMv", kind: "requestedOutput", symbol: "P1.Heater", unit: "%", span: 100 }],
    };
    assert.strictEqual(signalRoleName(sides, "P1.HeaterPosition"), "Output");
    assert.strictEqual(signalRoleName(sides, "P1.Heater"), "requestedMv");
    assert.strictEqual(signalRoleTitle(sides), "Measurement: P1.RoomC · Output: P1.HeaterPosition · requestedMv: P1.Heater");
  });

  test("durations show in their largest fitting unit", () => {
    assert.strictEqual(formatDurationSeconds(90), "90 s");
    assert.strictEqual(formatDurationSeconds(900), "15 min");
    assert.strictEqual(formatDurationSeconds(125), "2 min 5 s");
    assert.strictEqual(formatDurationSeconds(4500), "75 min");
    // Below 2 h the largest fitting unit is minutes (specification 35, B9.9).
    assert.strictEqual(formatDurationSeconds(5400), "90 min");
    assert.strictEqual(formatDurationSeconds(7200), "2 h");
    // A value below 1 ms is plain decimal text: the engine's duration reader refuses an exponent (specification 35, B9.9).
    assert.strictEqual(formatDurationSeconds(0.0005), "0.0005 s");
    assert.doesNotMatch(formatDurationSeconds(0.00000005), /e/i);
  });

  test("a failure shows a button only when the engine names its words (specification 35, A11.2)", () => {
    assert.strictEqual(nextActionHasButton({ kind: "approve", label: "Approve…" }), true, "the engine's Approve label is a button");
    assert.strictEqual(nextActionHasButton({ kind: "identify", label: "Identify plant" }), true);
    assert.strictEqual(nextActionHasButton({ kind: "wait", label: "" }), false, "a wait shows the message alone");
    assert.strictEqual(nextActionHasButton({ kind: "none", label: "" }), false);
  });

  test("the model shows the engine's key-numbers line, its fraction and its warnings", () => {
    const dom = fragment(`<main data-studio-three-page><div data-studio-model-result hidden><p data-studio-model-type></p>
      <p class="studio-model-equation" data-studio-model-equation></p><p class="studio-model-numbers" data-studio-model-numbers hidden></p>
      <ul class="studio-model-warnings" data-studio-model-warnings hidden></ul></div></main>`);
    try {
      const root = dom.document.querySelector<HTMLElement>("[data-studio-three-page]")!;
      // The words are the engine's (`modelType`, `keyNumbers` and `equationFraction`; the Rust tests of presentation.rs pin them): the page
      // shows each as it came and composes none.
      const presentation = {
        equation: "G(s) = 0.21 e^(-12s) / (4500s + 1)", modelType: "First-order model with delay",
        keyNumbers: "Gain 0.21 °C per %, time constant 75 min, delay 12 s",
        equationFraction: { numerator: "0.21", denominator: "(4500s + 1)", delay: "12s" },
        warnings: ["The response reached 87 % of its fitted final change."],
      } as unknown as IdentifiedModelPresentation;
      renderModelPresentation(root, presentation);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-type]")!.textContent, "First-order model with delay");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-numbers]")!.textContent, "Gain 0.21 °C per %, time constant 75 min, delay 12 s");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-numbers]")!.hidden, false);
      assert.strictEqual(root.querySelectorAll("[data-studio-model-warnings] li").length, 1);
      assert.strictEqual(root.querySelector("[data-studio-model-warnings] li")!.textContent, "The response reached 87 % of its fitted final change.");
      assert.strictEqual(root.querySelector(".studio-math-fraction .studio-math-num")!.textContent, "0.21", "the equation is a stacked fraction");
      assert.strictEqual(root.querySelector(".studio-math-fraction .studio-math-den")!.textContent, "(4500s + 1)");
      assert.strictEqual(root.querySelector("[data-studio-model-equation] sup")!.textContent, "−12s", "the delay is the exponent of e");
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-result]")!.hidden, false);
    } finally { dom.restore(); }
  });

  test("a model without a fraction shows the engine's equation text, and one without words shows no key numbers", () => {
    const dom = fragment(`<main data-studio-three-page><div data-studio-model-result hidden><p data-studio-model-type></p>
      <p class="studio-model-equation" data-studio-model-equation></p><p class="studio-model-numbers" data-studio-model-numbers></p>
      <ul class="studio-model-warnings" data-studio-model-warnings></ul></div></main>`);
    try {
      const root = dom.document.querySelector<HTMLElement>("[data-studio-three-page]")!;
      renderModelPresentation(root, { equation: "G(s) = 1 / (1s + 1)", warnings: [] } as unknown as IdentifiedModelPresentation);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-equation]")!.textContent, "G(s) = 1 / (1s + 1)");
      assert.strictEqual(root.querySelector(".studio-math-fraction"), null);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-numbers]")!.hidden, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-studio-model-warnings]")!.hidden, true);
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
      // The engine's answer fills the methods (specification 35, Tuning method); before it, the select holds Automatic alone.
      assert.deepStrictEqual(values, ["automatic"]);
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
      } as unknown as LoopDefinition;
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

  test("a duration goes back as the text it shows, and a number of seconds goes back as the number while its text is unchanged", () => {
    const dom = fragment("<main></main>");
    try {
      const submit = (fields: Parameters<typeof openCompactNumberEditor>[0]["fields"], type?: (input: HTMLInputElement) => void) => {
        let committed: Record<string, number | string> | undefined;
        const dialog = openCompactNumberEditor({ title: "Experiment limits", fieldAttribute: "data-studio-limit", fields, commit: values => { committed = { ...values }; } });
        const input = dialog.querySelector<HTMLInputElement>("input")!;
        const shown = input.value;
        if (type) type(input);
        dialog.querySelector("form")!.dispatchEvent(new dom.window.Event("submit", { bubbles: true, cancelable: true }));
        return { shown, committed };
      };
      const duration = (value: number | string) => [{ key: "durationSeconds", label: "Maximum duration", value, duration: true }];
      // A number of seconds is shown in its largest unit, and the number goes back while the text is unchanged (B9.9): a rounded display never edits
      // a stored value. 89.999 s shows as "90 s" and is not stored as 90.
      assert.deepStrictEqual(submit(duration(900)), { shown: "15 min", committed: { durationSeconds: 900 } });
      assert.deepStrictEqual(submit(duration(89.999)), { shown: "90 s", committed: { durationSeconds: 89.999 } });
      // The typed text is committed; the engine reads it as 120 s (Rust: a_typed_duration_reads_as_the_seconds_it_names).
      assert.deepStrictEqual(submit(duration(900), input => { input.value = "2 min"; }).committed, { durationSeconds: "2 min" });
      // A text is shown as it is and goes back as it is: the engine's words, or what the engineer typed. Wrong behaviour caught: a page that
      // formatted plain digits again would show and send "47.12 s" for the "47.123" the engineer typed, and "90 s" for "89.999".
      assert.deepStrictEqual(submit(duration("47.123")), { shown: "47.123", committed: { durationSeconds: "47.123" } });
      assert.deepStrictEqual(submit(duration("89.999")), { shown: "89.999", committed: { durationSeconds: "89.999" } });
      assert.deepStrictEqual(submit(duration("2 min 5 s")), { shown: "2 min 5 s", committed: { durationSeconds: "2 min 5 s" } });
    } finally { dom.restore(); }
  });

  test("the setup summary states what the code provides, and Change reveals its selector", () => {
    const model = {
      structure: "pid", addressedReal: ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"], booleans: ["P1.AutoMode"],
      records: ["P1.Parameters"], writes: [], initializes: [], pids: [{ instance: "P1.Controller", task: { name: "MainTask", intervalMs: 100 } }],
      instances: ["P1"], reals: ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"],
      prefill: { instance: "P1.Controller", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.AppliedPowerPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualOutput" },
      // The loop name the form starts from is the engine's (`loopNames`, B1.3): the last segment of the controller's instance path.
      loopNames: [{ instance: "P1.Controller", name: "Controller" }],
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

  test("Tune and the import wait for their gates, not for the engine's availability alone", () => {
    const dom = operationRoot();
    try {
      let gate: string | undefined = "The plant mapping is incomplete.";
      const operation = new ThreePageOperation(dom.root, noop, noop, noop, () => gate, noop);
      // The stage tabs keep the engine's stage answer (B9.4): the engine says Tune can start.
      const tab = dom.document.createElement("button");
      tab.dataset.engineerStageTab = "tune";
      dom.root.append(tab);
      applyStageStatuses(dom.root, [{ stage: "Tune", status: "ready", label: "Ready", available: true }]);
      const tune = dom.root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!;
      const importControl = dom.root.querySelector<HTMLInputElement>("[data-identify-import]")!;
      operation.synchronize();
      assert.strictEqual(tune.disabled, true, "an available stage does not override a closed gate");
      assert.strictEqual(importControl.disabled, true, "the idle lock keeps the import closed while its gate is closed");
      gate = undefined;
      operation.synchronize();
      assert.strictEqual(tune.disabled, false, "Tune opens once the engine and every gate agree");
      assert.strictEqual(importControl.disabled, false);
    } finally { dom.restore(); }
  });

  test("a stage action is enabled exactly when the engine's stage answer says it can start, whatever the page's other evidence", () => {
    const dom = operationRoot();
    try {
      const operation = new ThreePageOperation(dom.root, noop, noop, noop);
      for (const id of ["model", "tune", "verify"]) {
        const tab = dom.document.createElement("button");
        tab.dataset.engineerStageTab = id;
        dom.root.append(tab);
      }
      const tune = dom.root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!;
      const verify = dom.root.querySelector<HTMLButtonElement>("[data-engineer-verify]")!;
      const answer = (tuneAvailable: boolean, verifyAvailable: boolean) => [
        { stage: "Identify", status: "complete", label: "Complete", available: true },
        { stage: "Tune", status: tuneAvailable ? "ready" : "needs-data", label: "", available: tuneAvailable },
        { stage: "Validate", status: verifyAvailable ? "ready" : "needs-data", label: "", available: verifyAvailable },
      ];
      operation.synchronize();
      assert.deepStrictEqual([tune.disabled, verify.disabled], [true, true], "before the engine's first stage answer nothing starts");
      applyStageStatuses(dom.root, answer(true, false));
      operation.synchronize();
      assert.deepStrictEqual([tune.disabled, verify.disabled], [false, true], "a model makes Tune available, not Validate");
      // An answer of the engine that carries a candidate does not move the gate by itself: only the stage answer does.
      operation.receive("flow", { status: "completed", stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false } });
      assert.strictEqual(verify.disabled, true, "the evidence is not the gate");
      applyStageStatuses(dom.root, answer(true, true));
      operation.synchronize();
      assert.deepStrictEqual([tune.disabled, verify.disabled], [false, false], "a candidate makes Validate available");
      // A change that left only the validation not current leaves the candidate, so Validate stays available (A8.4).
      operation.receive("engineer", { status: "edited", reason: "Inputs changed. Tune and verify before applying.", stageEvidence: { hasCandidate: true } });
      assert.strictEqual(verify.disabled, false);
      assert.strictEqual(dom.root.querySelector<HTMLElement>('[data-studio-message="verify"]')!.textContent, "Inputs changed. Tune and verify before applying.",
        "the Validate page says why in the engine's words");
      // A stage the engine says cannot start (it needs data, or it does not apply) never starts, and a chosen analysis starts from Tune whatever the stage says.
      applyStageStatuses(dom.root, answer(false, false));
      operation.synchronize();
      assert.deepStrictEqual([tune.disabled, verify.disabled], [true, true]);
      dom.root.dataset.analysisSelected = "true";
      operation.synchronize();
      assert.strictEqual(tune.disabled, false, "an analysis shown on the Tune page runs from the Tune button");
      assert.strictEqual(verify.disabled, true);
    } finally { dom.restore(); }
  });

  test("a stage that cannot run yet shows the engine's reason at the top of its page, the template writes none, and a later answer clears it", () => {
    const html = renderThreePageStudioHtml({ cspSource: "x", nonce: "n", chartScript: "chart.js", workflowScript: "workflow.js", project: "p",
      session: "s", loop: "l", loopRevision: "r", revision: "v", processUnit: "PV", outputUnit: "MV" } as unknown as ControlStudioView, text => text);
    const dom = install(html);
    try {
      const root = dom.document.querySelector<HTMLElement>("[data-studio-three-page]")!;
      // The page's own sentences ("Identify a plant model first.", "Tune a controller first.") are gone: its action areas start empty.
      for (const page of ["tune", "verify"]) {
        const area = root.querySelector<HTMLElement>(`[data-studio-message="${page}"]`)!;
        assert.deepStrictEqual([area.textContent, area.hidden], ["", true], page);
      }
      const reasonOf = (page: string) => root.querySelector<HTMLElement>(`[data-studio-page="${page}"] > [data-engineer-stage-reason]`)?.textContent ?? null;
      applyStageStatuses(root, [
        { stage: "Tune", status: "needs-data", label: "Needs data", available: false, reason: "The engine's words for a missing model." },
        { stage: "Validate", status: "needs-data", label: "Needs data", available: false, reason: "The engine's words for a missing candidate." }]);
      assert.deepStrictEqual([reasonOf("tune"), reasonOf("verify")], ["The engine's words for a missing model.", "The engine's words for a missing candidate."]);
      assert.strictEqual(root.querySelector('[data-studio-page="tune"]')!.hasAttribute("inert"), false, "a stage that waits is not inert: only Not applicable is");
      // A stage that can start names none, so a reopened study with a model shows no sentence on Tune (the audit's N6).
      applyStageStatuses(root, [
        { stage: "Tune", status: "ready", label: "Ready", available: true },
        { stage: "Validate", status: "needs-data", label: "Needs data", available: false, reason: "The engine's words for a missing candidate." }]);
      assert.deepStrictEqual([reasonOf("tune"), reasonOf("verify")], [null, "The engine's words for a missing candidate."]);
    } finally { dom.restore(); }
  });

  test("a stage the engine answers Not applicable shows the engine's reason at the top of its page, and a later answer clears it", () => {
    const dom = operationRoot();
    try {
      const tab = dom.document.createElement("button");
      tab.dataset.engineerStageTab = "tune";
      dom.root.append(tab);
      const page = dom.root.querySelector<HTMLElement>('[data-studio-page="tune"]')!;
      const tune = dom.root.querySelector<HTMLButtonElement>("[data-engineer-tune]")!;
      const operation = new ThreePageOperation(dom.root, noop, noop, noop);
      applyStageStatuses(dom.root, [{ stage: "Tune", status: "not-applicable", label: "Not applicable", available: false, reason: "This loop declares no controller law." }]);
      operation.synchronize();
      assert.strictEqual(page.querySelector<HTMLElement>(":scope > [data-engineer-stage-reason]")?.textContent, "This loop declares no controller law.",
        "the engine's reason is shown in its words, in place of the page's actions");
      assert.strictEqual(page.hasAttribute("inert"), true, "the page's controls are inert");
      assert.strictEqual(tune.disabled, true, "and the stage's action cannot start");
      assert.strictEqual(tab.getAttribute("aria-label"), "Tune, Not applicable");
      applyStageStatuses(dom.root, [{ stage: "Tune", status: "ready", label: "Ready", available: true }]);
      operation.synchronize();
      assert.strictEqual(page.querySelector("[data-engineer-stage-reason]"), null, "any other status clears the reason");
      assert.strictEqual(page.hasAttribute("inert"), false);
      assert.strictEqual(tune.disabled, false);
    } finally { dom.restore(); }
  });

  test("Done commits what was typed: a number stays a number, a blank entry goes as its text, and a blank optional field is left out", () => {
    const dom = fragment("<main></main>");
    try {
      const fields = [{ key: "inputMinimum", label: "Minimum" }, { key: "outputMaximum", label: "Maximum", value: 80 },
        { key: "durationSeconds", label: "Maximum duration", duration: true }, { key: "load", label: "Load", optional: true }];
      let committed: Record<string, number | string> | undefined;
      const dialog = openCompactNumberEditor({ title: "Experiment limits", fieldAttribute: "data-studio-limit", fields,
        commit: values => { committed = { ...values }; } });
      dialog.querySelector("form")!.dispatchEvent(new dom.window.Event("submit", { bubbles: true, cancelable: true }));
      // The engine reads each text and refuses a blank one under its field (B3.2), so the editor refuses none: a page that refused a blank entry
      // itself would show a sentence of its own, and the engine's would never be seen.
      assert.deepStrictEqual(committed, { inputMinimum: "", outputMaximum: 80, durationSeconds: "" },
        "a blank field and a blank duration go as typed, a number stays a number, and a blank optional field is left out");
      assert.deepStrictEqual(Array.from(dialog.querySelectorAll<HTMLElement>("[role=alert]")).filter(notice => !notice.hidden), [], "the editor marks no field");
    } finally { dom.restore(); }
  });
});
