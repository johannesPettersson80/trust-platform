import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { runLockedControls } from "../../controlStudio/webview/runLock";
import { ThreePageOperation } from "../../controlStudio/webview/threePageOperation";
import { ThreePageApplication } from "../../controlStudio/webview/threePageApplication";
import { applyStageStatuses } from "../../controlStudio/webview/engineShared";
import { ThreePageBindings } from "../../controlStudio/webview/threePageBindings";
import { ThreePageController } from "../../controlStudio/webview/threePageController";
import { ThreePageIdentification } from "../../controlStudio/webview/threePageIdentification";
import { ThreePageExperiments } from "../../controlStudio/webview/threePageExperiments";
import { renderThreePageIdentificationTemplate } from "../../controlStudio/threePageIdentificationTemplate";

const { JSDOM } = require("jsdom");

type Post = { kind: string; payload?: Record<string, unknown> };

/** The stage actions, notices and fields that the three pages render; each test adds only what its module reads. */
const STAGE_MARKUP = `
  <button data-engineer-find>Identify plant</button>
  <button data-identify-import>Open recording…</button>
  <button data-studio-reset-chart>Reset</button>
  <button data-studio-limits>Limits…</button>
  <button data-identify-cancel hidden>Cancel</button>
  <p data-studio-message="model" hidden></p>
  <select data-engineer-acquisition-method><option value="automatic">Automatic</option><option value="recorded-fit">Recorded-data fit</option><option value="project">Project experiment</option></select>
  <select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="SIMC">SIMC</option><option value="AMIGO">AMIGO</option><option value="manual">Manual</option></select>
  <button data-engineer-tune disabled>Tune controller</button>
  <button data-studio-requirements>Requirements…</button>
  <button data-engineer-channels>Channels…</button>
  <textarea data-state-design-field="desiredPoles"></textarea>
  <p data-studio-message="tune" hidden></p>
  <button data-engineer-verify disabled>Validate controller</button>
  <p data-studio-message="verify" hidden></p>
  <button data-engineer-apply disabled>Apply</button>
`;

/** Values that satisfy the single-output requirements editor without any optional row. */
const REQUIREMENTS = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 };

function fixtureLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
}

/** A configured target that the bindings module can offer for the fixture's plant mapping. */
function target(value: any, loopId: string): any {
  return {
    loopId,
    name: value.name,
    configurationPath: `${loopId}.json`,
    controller: { instance: value.controller.instance, type: value.controller.type, tasks: [{ name: "MainTask", intervalMs: value.controller.taskPeriodMs }] },
    inputs: [{ id: "manipulatedVariable", symbol: value.signals.manipulatedVariable.symbol, address: "%QL0", iecType: "LREAL", unit: "V", bounds: { minimum: -20, maximum: 20 }, role: "MV", order: 0 }],
    outputs: [{ id: "processVariable", symbol: value.signals.processVariable.symbol, address: "%IL0", iecType: "LREAL", unit: "m", bounds: { minimum: -20, maximum: 20 }, role: "PV", order: 0 }],
    controllerCompatibility: { kind: "accepted", families: ["P", "PI", "PD", "PID"] },
  };
}

/** Runs one check on a fresh page, restoring the globals that the webview modules read. */
function withPage(markup: string, check: (root: HTMLElement, window: any) => void): void {
  const dom = new JSDOM(`<main data-studio-three-page>${markup}</main>`);
  const { window } = dom;
  const saved = {
    document: (globalThis as any).document, window: (globalThis as any).window, HTMLElement: (globalThis as any).HTMLElement,
    CustomEvent: (globalThis as any).CustomEvent, Option: (globalThis as any).Option,
  };
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, CustomEvent: window.CustomEvent, Option: window.Option });
  // jsdom has no modal dialog; the editors need only the open state and the close event.
  window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new window.Event("close")); };
  try {
    check(window.document.querySelector("main") as HTMLElement, window);
  } finally {
    Object.assign(globalThis, saved);
    window.close();
  }
}

function control<T extends Element = HTMLElement>(scope: ParentNode, selector: string): T {
  const found = scope.querySelector<T>(selector);
  assert.ok(found, `missing ${selector}`);
  return found;
}

function disabled(scope: ParentNode, selector: string): boolean {
  return control<HTMLButtonElement>(scope, selector).disabled;
}

function buttonNamed(scope: ParentNode, text: string): HTMLButtonElement {
  const found = Array.from(scope.querySelectorAll<HTMLButtonElement>("button")).find(button => button.textContent === text);
  assert.ok(found, `missing button ${text}`);
  return found;
}

function submitRequirements(window: any): void {
  const form = window.document.querySelector("[data-trust-compact-editor] form") as HTMLFormElement | null;
  assert.ok(form, "the requirements editor is open");
  form.dispatchEvent(new window.Event("submit", { cancelable: true }));
}

const RUN_LOCKED = ["textarea", "[data-engineer-channels]", "[data-studio-limits]", "[data-studio-requirements]", "[data-studio-reset-chart]", "[data-identify-import]", "[data-engineer-acquisition-method]"];

/** The engine's reason of the block of an Apply's checkpoint that waits for its Restore (trust-control `checkpoint_block::UNRESOLVED_APPLY`). */
const BLOCK_REASON = "An Apply or a Restore of this loop stopped before the runtime confirmed its end. Restore previous settings first: until the runtime confirms it, the loop's other actions wait.";

/** The offers of a Validate answer that offers Apply, as the Validate stage relays them to the Apply area (B5.4, B7). */
const OFFERED = { status: "offers", engineOutcome: { applyAvailable: true, primaryAction: "apply" } };

suite("Control Studio three-page locks and confirmations", () => {
  test("cancelling the Requirements editor that Tune opened never lets a later manual save start Tune", () => {
    withPage("", (_root, window) => {
      const events: string[] = [];
      editControlRequirements(fixtureLoop(), { ...REQUIREMENTS }, () => events.push("tune editor saved"), undefined, () => events.push("resume Tune"));
      control<HTMLButtonElement>(window.document, "[data-studio-editor-cancel]").click();
      assert.deepStrictEqual(events, [] as string[], "Cancel discards the edit and leaves the Tune start paused");
      editControlRequirements(fixtureLoop(), { ...REQUIREMENTS }, () => events.push("manual editor saved"));
      submitRequirements(window);
      assert.deepStrictEqual(events, ["manual editor saved"], "a later manual save must not resume the cancelled Tune start");
    });
  });

  test("saving the Requirements editor that Tune opened commits the requirements, then resumes the paused start", () => {
    withPage("", (_root, window) => {
      const events: string[] = [];
      editControlRequirements(fixtureLoop(), { ...REQUIREMENTS }, () => events.push("commit"), undefined, () => events.push("resume"));
      submitRequirements(window);
      assert.deepStrictEqual(events, ["commit", "resume"]);
    });
  });

  test("the run lock covers fields, state-design text areas and dialog openers, but never a stage action or the Abort control", () => {
    withPage(STAGE_MARKUP, root => {
      const locked = new Set<Element>(runLockedControls(root));
      for (const selector of RUN_LOCKED) assert.ok(locked.has(control(root, selector)), `${selector} is locked during a run`);
      for (const selector of ["[data-engineer-find]", "[data-engineer-tune]", "[data-engineer-verify]", "[data-engineer-apply]", "[data-identify-cancel]"]) {
        assert.ok(!locked.has(control(root, selector)), `${selector} is not a run-lock control`);
      }
    });
  });

  test("a running operation locks the text areas, Channels and dialog openers while its Abort stays enabled", () => {
    withPage(STAGE_MARKUP, root => {
      const posts: Post[] = [];
      const operation = new ThreePageOperation(root, (kind, payload) => posts.push({ kind, payload }), () => undefined, () => undefined,
        () => "Choose the complete input and output mapping of a configured loop.");
      operation.start("find", {});
      assert.strictEqual(control<HTMLButtonElement>(root, "[data-engineer-find]").textContent, "Abort");
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false, "Abort stays reachable even when a gate reports a block");
      for (const selector of RUN_LOCKED) assert.strictEqual(disabled(root, selector), true, `${selector} is locked during the run`);
      operation.receive("flow", { status: "completed" });
      assert.strictEqual(disabled(root, "textarea"), false, "the lock ends with the operation");
      assert.strictEqual(disabled(root, "[data-engineer-channels]"), false, "Channels is available again after the operation");
    });
  });

  test("an Apply run locks the same controls while its stage Abort stays enabled", () => {
    // The Apply area keeps the Apply stage of the engine's last answer (engineShared.ts `applyStageStatuses`).
    withPage(`${STAGE_MARKUP}<div data-studio-application></div>`, root => {
      const posts: Post[] = [];
      const application = new ThreePageApplication(root, (kind, payload) => posts.push({ kind, payload }), () => false, () => undefined);
      // A Validate answer that offers Apply: its Apply stage can start (B7, B9.4), and its offers name Apply.
      applyStageStatuses(root, [{ stage: "Apply", status: "ready", label: "Ready", available: true }]);
      application.receive(OFFERED);
      control<HTMLButtonElement>(root, "[data-engineer-apply]").click();
      assert.deepStrictEqual(posts.map(post => post.kind), ["control-studio.application.apply"]);
      for (const selector of RUN_LOCKED) assert.strictEqual(disabled(root, selector), true, `${selector} is locked while Apply runs`);
      assert.strictEqual(control<HTMLButtonElement>(root, "[data-engineer-find]").textContent, "Abort");
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false);
    });
  });

  test("the engine's block holds the stage actions and names its reason on their pages, while Restore holds the position", () => {
    // B6.4, B6.7: while an Apply's checkpoint waits for its Restore, the engine's stage statuses hold the stages that write and name the
    // reason, and its offers put Restore previous settings in the position. The page renders both. The editors of what the stages read and
    // Reset stay usable: the engine refuses a change of the study inputs and a Reset with the block's reason (B6.4), so the page holds no
    // lock of its own. The confirmed Restore's stages end the block.
    const markup = `${STAGE_MARKUP}
      <button data-engineer-stage-tab="model">Identify</button><button data-engineer-stage-tab="tune">Tune</button><button data-engineer-stage-tab="verify">Validate</button>
      <section data-studio-page="model"></section><section data-studio-page="tune"></section><section data-studio-page="verify"><div data-studio-application></div></section>`;
    withPage(markup, root => {
      const posts: Post[] = [];
      const post = (kind: string, payload?: Record<string, unknown>) => { posts.push({ kind, payload }); };
      const operation = new ThreePageOperation(root, post, () => undefined, () => undefined);
      const application = new ThreePageApplication(root, post, () => operation.busy, () => undefined);
      const held = (stage: string, status: string) => ({ stage, status, label: status, available: false, reason: BLOCK_REASON });
      applyStageStatuses(root, [held("Identify", "complete"), held("Tune", "complete"), held("Validate", "complete"), held("Apply", "ready")]);
      application.receive({ status: "opened", applyObservationResumable: false,
        engineOutcome: { applyAvailable: false, restoreAvailable: true, keepAvailable: false, primaryAction: "restore" } });
      operation.synchronize();
      assert.deepStrictEqual([disabled(root, "[data-engineer-find]"), disabled(root, "[data-engineer-tune]"), disabled(root, "[data-engineer-verify]")],
        [true, true, true], "each stage the engine holds cannot start");
      for (const page of ["model", "tune", "verify"]) {
        const section = control<HTMLElement>(root, `[data-studio-page="${page}"]`);
        assert.strictEqual(section.querySelector<HTMLElement>(":scope > [data-engineer-stage-reason]")?.textContent, BLOCK_REASON, page);
        assert.strictEqual(section.hasAttribute("inert"), false, `${page} stays usable, so Restore stays in reach`);
      }
      const apply = control<HTMLButtonElement>(root, "[data-engineer-apply]");
      assert.deepStrictEqual([apply.textContent, apply.disabled], ["Restore previous settings", false]);
      for (const selector of RUN_LOCKED) {
        assert.strictEqual(disabled(root, selector), false, `${selector} stays usable: the engine refuses its change while the checkpoint waits`);
      }
      apply.click();
      assert.deepStrictEqual(posts.map(sent => sent.kind), ["control-studio.application.restore"], "Restore asks the engine for its review");
      // The confirmed Restore answers the stages without the block, and offers nothing (B6.4, B7).
      application.receive({ status: "commissioned", applicationAction: "restore", reason: "Restored.",
        engineOutcome: { applyAvailable: false, restoreAvailable: false, keepAvailable: false } });
      const free = (stage: string, status: string, available: boolean) => ({ stage, status, label: status, available });
      applyStageStatuses(root, [free("Identify", "complete", true), free("Tune", "complete", true), free("Validate", "complete", true), free("Apply", "ready", true)]);
      operation.synchronize();
      assert.deepStrictEqual([disabled(root, "[data-engineer-find]"), disabled(root, "[data-engineer-tune]"), disabled(root, "[data-engineer-verify]")],
        [false, false, false]);
      assert.strictEqual(root.querySelector("[data-engineer-stage-reason]"), null, "the block's reason leaves every page");
      for (const selector of RUN_LOCKED) assert.strictEqual(disabled(root, selector), false, `${selector} is free again`);
      assert.deepStrictEqual([apply.textContent, apply.disabled], ["Apply", true], "nothing is offered until a validation passes again");
    });
  });

  test("a Method change keeps Identify disabled while the plant mapping is incomplete, and releases it once the mapping is valid", () => {
    const markup = `${STAGE_MARKUP}
      <select data-studio-controller><option value="Main">Main · Control.PID</option></select>
      <select data-studio-inputs multiple></select><select data-studio-outputs multiple></select>
      ${renderThreePageIdentificationTemplate()}
      <section data-three-page-experiment hidden><select data-three-page-experiment-signals multiple></select><p data-three-page-experiment-status></p><div data-three-page-experiment-plan hidden></div></section>`;
    withPage(markup, (root, window) => {
      const loop = fixtureLoop();
      const posts: Post[] = [];
      const post = (kind: string, payload: Record<string, unknown> = {}) => posts.push({ kind, payload });
      let bindingsBlock: string | undefined;
      const operation = new ThreePageOperation(root, post, () => undefined, () => undefined, () => bindingsBlock);
      // The controller selector binds its own listeners on the page; the bindings no longer take it.
      const _controller = new ThreePageController(root, loop, post);
      const bindings = new (ThreePageBindings as any)(root, loop, post, () => undefined,
        (valid: boolean, reason?: string) => { bindingsBlock = valid ? undefined : reason; operation.synchronize(); });
      const identification = new ThreePageIdentification(root, post, () => operation.stageBlock());
      const experiments = new ThreePageExperiments(root, {} as any, post, () => operation.stageBlock());
      // The engine answers each mapping the selectors name (B1.2, B3.6): the current loop's mapping first.
      const mapping = (outcome: string, sentence = "", blocksRun = false) => ({ outcome, changed: false, blocksRun, sentence, options: [] });
      bindings.receive({ status: "bindings", targets: [target(loop, loop.id)], mapping: mapping("current") });
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false, "a complete mapping allows Identify");

      const input = control<HTMLSelectElement>(root, "[data-studio-inputs]");
      const output = control<HTMLSelectElement>(root, "[data-studio-outputs]");
      const other = new window.Option("Other MV (V)", "Main.OtherMV");
      input.append(other);
      Array.from(input.options).forEach(option => { option.selected = option === other; });
      output.options[0].selected = true;
      input.dispatchEvent(new window.Event("change", { bubbles: true }));
      bindings.receive({ status: "bindings", targets: [target(loop, loop.id)], mapping: mapping("unmatched", "Choose the complete input and output mapping of a configured loop.", true) });
      assert.strictEqual(bindings.canRun, false, "the selected plant mapping matches no configured loop");
      assert.strictEqual(disabled(root, "[data-engineer-find]"), true, "an incomplete mapping blocks Identify");

      // The Method change handler re-renders the stage actions, then each method-specific owner renders its own state.
      operation.synchronize();
      assert.strictEqual(disabled(root, "[data-engineer-find]"), true, "Automatic keeps Identify disabled");
      identification.setSelectedMethod(true);
      assert.strictEqual(disabled(root, "[data-engineer-find]"), true, "Recorded-data fit keeps Identify disabled");
      experiments.setSelectedMethod(true);
      assert.strictEqual(disabled(root, "[data-engineer-find]"), true, "Project experiment keeps Identify disabled");
      assert.strictEqual(disabled(root, "[data-engineer-tune]"), true, "Tune stays blocked by the same gate");

      Array.from(input.options).forEach(option => { option.selected = option.value === loop.signals.manipulatedVariable.symbol; });
      input.dispatchEvent(new window.Event("change", { bubbles: true }));
      bindings.receive({ status: "bindings", targets: [target(loop, loop.id)], mapping: mapping("current") });
      assert.strictEqual(bindings.canRun, true);
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false, "a complete mapping releases Identify");
    });
  });

  test("Reset changes nothing until the engineer confirms it in the page", () => {
    withPage(STAGE_MARKUP, root => {
      const posts: Post[] = [];
      const operation = new ThreePageOperation(root, (kind, payload) => posts.push({ kind, payload }), () => undefined, () => undefined);
      const reset = control<HTMLButtonElement>(root, "[data-studio-reset-chart]");
      operation.bindReset(reset);
      const row = control<HTMLElement>(root, ".studio-reset-confirm");
      assert.strictEqual(row.hidden, true, "the confirmation starts closed");
      reset.click();
      assert.strictEqual(row.hidden, false, "Reset opens the confirmation");
      assert.match(row.textContent ?? "", /Reset starts a new study\. The model, the tuned controller and the validation results move to the study's history\./,
        "the confirmation says what Reset does (A8.4): the study moves to history, it is not discarded");
      assert.deepStrictEqual(posts, [], "opening the confirmation posts nothing");
      buttonNamed(row, "Keep").click();
      assert.strictEqual(row.hidden, true, "Keep closes the confirmation");
      assert.deepStrictEqual(posts, [], "Keep leaves the study untouched");
      reset.click();
      buttonNamed(row, "Reset study").click();
      assert.deepStrictEqual(posts, [{ kind: "control-studio.study.reset", payload: undefined }], "only Reset study posts the reset");
      assert.strictEqual(row.hidden, true);
    });
  });

  test("a confirmed Reset is sent whatever the page holds, and the engine's refusal is shown with its next action", () => {
    // The engine refuses Reset while an operation runs or an Apply's checkpoint waits (B6.7); the page writes no refusal of its own.
    const markup = `${STAGE_MARKUP}<p data-studio-failure-detail="model" hidden></p><button data-studio-failure-action="model" hidden></button>`;
    withPage(markup, root => {
      const posts: Post[] = [];
      const operation = new ThreePageOperation(root, (kind, payload) => posts.push({ kind, payload }), () => undefined, () => undefined);
      const reset = control<HTMLButtonElement>(root, "[data-studio-reset-chart]");
      operation.bindReset(reset);
      reset.click();
      buttonNamed(control(root, ".studio-reset-confirm"), "Reset study").click();
      assert.deepStrictEqual(posts, [{ kind: "control-studio.study.reset", payload: undefined }], "the page refuses nothing itself");
      assert.strictEqual(reset.disabled, true, "the button waits while the page's own Reset is in flight");
      operation.receive("flow", { status: "failed", failure: { message: "An Apply or a Restore of this loop stopped before the runtime confirmed its end.",
        detail: BLOCK_REASON, nextAction: { kind: "restore", label: "Restore previous settings" } } });
      assert.strictEqual(control<HTMLElement>(root, "[data-studio-message=\"model\"]").textContent,
        "An Apply or a Restore of this loop stopped before the runtime confirmed its end.", "the engine's sentence");
      const action = control<HTMLButtonElement>(root, "[data-studio-failure-action=\"model\"]");
      assert.deepStrictEqual([action.hidden, action.textContent], [false, "Restore previous settings"], "the engine's next action is a control");
      assert.strictEqual(reset.disabled, false, "the refused Reset has ended, so the button is free again");
    });
  });
});
