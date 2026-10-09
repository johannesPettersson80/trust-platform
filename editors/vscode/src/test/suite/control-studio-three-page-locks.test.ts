import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { runLockedControls } from "../../controlStudio/webview/runLock";
import { ThreePageOperation } from "../../controlStudio/webview/threePageOperation";
import { ThreePageApplication } from "../../controlStudio/webview/threePageApplication";
import { ThreePageRecovery } from "../../controlStudio/webview/threePageRecovery";
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
    withPage(STAGE_MARKUP, root => {
      const posts: Post[] = [];
      const application = new ThreePageApplication(root, (kind, payload) => posts.push({ kind, payload }), () => false, () => undefined);
      application.receive({ status: "verified" });
      control<HTMLButtonElement>(root, "[data-engineer-apply]").click();
      assert.deepStrictEqual(posts.map(post => post.kind), ["control-studio.application.apply"]);
      for (const selector of RUN_LOCKED) assert.strictEqual(disabled(root, selector), true, `${selector} is locked while Apply runs`);
      assert.strictEqual(control<HTMLButtonElement>(root, "[data-engineer-find]").textContent, "Abort");
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false);
    });
  });

  test("recovery locks the same controls and restores their earlier state once the operation is reconciled", () => {
    withPage(STAGE_MARKUP, root => {
      const recovery = new ThreePageRecovery(root, () => undefined, () => false);
      recovery.receive({ status: "recovery-required", cancellationUnconfirmed: true, reason: "Reconnect the original simulator and confirm cancellation." });
      for (const selector of RUN_LOCKED) assert.strictEqual(disabled(root, selector), true, `${selector} is locked during recovery`);
      recovery.receive({ status: "failed", recoveryResolved: true, reason: "No operation was replayed." });
      for (const selector of RUN_LOCKED) assert.strictEqual(disabled(root, selector), false, `${selector} is restored after reconciliation`);
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
      const controller = new ThreePageController(root, loop, post);
      const bindings = new (ThreePageBindings as any)(root, loop, controller, post, () => undefined,
        (valid: boolean, reason?: string) => { bindingsBlock = valid ? undefined : reason; operation.synchronize(); });
      const identification = new ThreePageIdentification(root, post, undefined, () => operation.stageBlock());
      const experiments = new ThreePageExperiments(root, {} as any, post, () => operation.stageBlock());
      bindings.receive({ status: "bindings", targets: [target(loop, loop.id)] });
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false, "a complete mapping allows Identify");

      const input = control<HTMLSelectElement>(root, "[data-studio-inputs]");
      const output = control<HTMLSelectElement>(root, "[data-studio-outputs]");
      const other = new window.Option("Other MV (V)", "Main.OtherMV");
      input.append(other);
      Array.from(input.options).forEach(option => { option.selected = option === other; });
      output.options[0].selected = true;
      input.dispatchEvent(new window.Event("change", { bubbles: true }));
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
      assert.strictEqual(bindings.canRun, true);
      assert.strictEqual(disabled(root, "[data-engineer-find]"), false, "a complete mapping releases Identify");
    });
  });

  test("Reset discards nothing until the engineer confirms it in the page", () => {
    withPage(STAGE_MARKUP, root => {
      const posts: Post[] = [];
      const operation = new ThreePageOperation(root, (kind, payload) => posts.push({ kind, payload }), () => undefined, () => undefined);
      const reset = control<HTMLButtonElement>(root, "[data-studio-reset-chart]");
      operation.bindReset(reset, () => false);
      const row = control<HTMLElement>(root, ".studio-reset-confirm");
      assert.strictEqual(row.hidden, true, "the confirmation starts closed");
      reset.click();
      assert.strictEqual(row.hidden, false, "Reset opens the confirmation");
      assert.match(row.textContent ?? "", /Reset discards the model, the tuned controller and the validation results\./);
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

  test("Reset is refused while the study is blocked, and the page says why", () => {
    withPage(STAGE_MARKUP, root => {
      const posts: Post[] = [];
      const operation = new ThreePageOperation(root, (kind, payload) => posts.push({ kind, payload }), () => undefined, () => undefined);
      const reset = control<HTMLButtonElement>(root, "[data-studio-reset-chart]");
      operation.bindReset(reset, () => true);
      reset.click();
      assert.strictEqual(control<HTMLElement>(root, ".studio-reset-confirm").hidden, true, "no confirmation opens");
      assert.strictEqual(control<HTMLElement>(root, "[data-studio-message=\"model\"]").textContent, "Finish or abort the current operation before resetting the study.");
      assert.deepStrictEqual(posts, []);
    });
  });
});
