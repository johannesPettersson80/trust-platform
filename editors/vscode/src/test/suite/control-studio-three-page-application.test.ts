import * as assert from "assert";
import { applyStageStatuses } from "../../controlStudio/webview/engineShared";
import { ThreePageApplication } from "../../controlStudio/webview/threePageApplication";

const { JSDOM } = require("jsdom");

function setup() {
  const dom = new JSDOM(`<main data-studio-three-page>
    <section data-studio-page="verify"><div data-studio-application>
      <p data-studio-message="verify"></p><button class="trust-button trust-button--primary" data-engineer-apply>Apply</button>
      <button class="trust-button" data-engineer-keep hidden disabled>Keep in project</button>
    </div></section>
    <select><option>PID</option></select><input data-studio-requirements>
    <button data-engineer-find>Identify plant</button><button data-engineer-tune>Tune controller</button>
    <button data-engineer-verify>Validate controller</button><input data-identify-import>
  </main>`);
  const root = dom.window.document.querySelector("[data-studio-three-page]") as HTMLElement;
  const previous = { document: (globalThis as any).document, window: (globalThis as any).window,
    HTMLButtonElement: (globalThis as any).HTMLButtonElement, HTMLSelectElement: (globalThis as any).HTMLSelectElement,
    HTMLInputElement: (globalThis as any).HTMLInputElement };
  Object.assign(globalThis, { document: dom.window.document, window: dom.window,
    HTMLButtonElement: dom.window.HTMLButtonElement, HTMLSelectElement: dom.window.HTMLSelectElement,
    HTMLInputElement: dom.window.HTMLInputElement });
  return { dom, root, button: root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!,
    keep: root.querySelector<HTMLButtonElement>("[data-engineer-keep]")!, notice: root.querySelector<HTMLElement>("[data-studio-message=verify]")!, previous };
}

function restore(state: ReturnType<typeof setup>): void {
  Object.assign(globalThis, state.previous);
  state.dom.window.close();
}

/**
 * The page's application over `state`: its posts go to `posts`, its notices to the verify message, its failures to `failures`. The page
 * renders it once when it starts (threePageWorkflow.ts synchronizes every module), so nothing is offered before the engine says so.
 */
function applicationOf(state: ReturnType<typeof setup>, posts: string[] = [], failures: string[] = []): ThreePageApplication {
  const application = new ThreePageApplication(state.root, kind => { posts.push(kind); }, () => false, message => { state.notice.textContent = message; },
    failure => { failures.push(failure.message); state.notice.textContent = failure.message; });
  application.synchronize();
  return application;
}

/** The offers of a Validate answer that offers Apply, as the Validate stage relays them (engineApplication.ts `offersPayload`, B5.4, B7). */
const OFFERED = { status: "offers", engineOutcome: { applyAvailable: true, primaryAction: "apply" } };

/**
 * One engine answer as the page receives it: an answer that states the Apply offer also carries the stages, whose Apply stage can start
 * exactly when Apply is offered (B7, B9.4), and the page shows them (engineShared.ts `applyStageStatuses`) before the Apply area takes
 * the answer. An answer that states no Apply offer, such as the end of a Keep or a status of the page's own request, carries no Apply stage.
 */
function answered(state: ReturnType<typeof setup>, application: ThreePageApplication, payload: Record<string, unknown>): void {
  const offers = payload.engineOutcome as Record<string, unknown> | undefined;
  if (typeof offers?.applyAvailable === "boolean") {
    applyStageStatuses(state.root, [{ stage: "Apply", status: "ready", label: "Ready", available: offers.applyAvailable }]);
  }
  application.receive(payload);
}

/** The end of an Apply that left its settings standing, as the engine offers what follows and names the primary action (B6.3, B6.5, B7). */
const APPLIED = { status: "applied", applicationAction: "apply", reason: "Applied: Kp 3.",
  engineOutcome: { applyAvailable: false, restoreAvailable: true, keepAvailable: true, primaryAction: "keep" } };

suite("Control Studio three-page application projection", () => {
  test("does not enable Apply until the engine offers it, whatever the page's evidence or status words say", () => {
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, { status: "verified", stageEvidence: { validationPassed: true } });
      assert.strictEqual(state.button.disabled, true, "the page reads no offer from a status word or the evidence");
      answered(state, application, { status: "offers", engineOutcome: { applyAvailable: false } });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", true],
        "a Validate answer that offers nothing leaves the position unavailable");
    } finally { restore(state); }
  });

  test("a Validate answer that offers Apply enables it, and the press waits for the engine's review", () => {
    const state = setup();
    try {
      const posts: string[] = [];
      const application = applicationOf(state, posts);
      assert.strictEqual(state.button.disabled, true, "nothing is offered before the engine says so");
      answered(state, application, OFFERED);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", false]);
      state.button.click();
      assert.deepStrictEqual(posts, ["control-studio.application.apply"]);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Applying…", true]);
    } finally { restore(state); }
  });

  test("Apply follows the Apply stage of the answer the page received last, and a run's start or the evidence withdraws nothing", () => {
    // B7, B9.4: every answer with stages renews whether the Apply stage can start (engineShared.ts `applyStageStatuses`), and in an answer
    // that states the offers it is exactly the Apply offer; an answer that leaves no validation current, such as a Tune's, answers it false
    // (A8.4). The page combines no two answers and withdraws nothing on the evidence or a status word of its own.
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, OFFERED);
      answered(state, application, { status: "running", flowStage: "Tune", reason: "Tuning controller…" });
      answered(state, application, { status: "candidate", stageEvidence: { validationPassed: false } });
      answered(state, application, { status: "edited", stageEvidence: { validationPassed: false }, validationInvalidated: true });
      assert.strictEqual(state.button.disabled, false, "the page withdraws nothing the engine has not withdrawn");
      applyStageStatuses(state.root, [{ stage: "Apply", status: "needs-data", label: "Needs data", available: false }]);
      application.synchronize();
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", true], "the engine's Apply stage cannot start");
      applyStageStatuses(state.root, [{ stage: "Apply", status: "ready", label: "Ready", available: true }]);
      application.synchronize();
      assert.strictEqual(state.button.disabled, false, "the Apply stage of the last answer can start again");
    } finally { restore(state); }
  });

  test("shows Apply's progress while it waits for the process to settle, and a cancelled wait frees the button with the engine's sentence", () => {
    const state = setup();
    try {
      const posts: string[] = [];
      const application = applicationOf(state, posts);
      answered(state, application, OFFERED);
      state.button.click();
      answered(state, application, { status: "running", applicationAction: "apply", reason: "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…" });
      assert.strictEqual(state.notice.textContent, "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…");
      assert.strictEqual(state.button.textContent, "Applying…");
      answered(state, application, { status: "cancelled", applicationAction: "apply", reason: "The operation was cancelled." });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", false], "the validated candidate stays applicable");
      assert.strictEqual(state.notice.textContent, "The operation was cancelled.");
      answered(state, application, { status: "cancelled", applicationAction: "apply", reason: "" });
      assert.strictEqual(state.notice.textContent, "", "the page writes no sentence of its own for a cancellation");
    } finally { restore(state); }
  });

  test("a refused Apply shows the engine's failure and leaves the engine's offers as they were", () => {
    const state = setup();
    try {
      const failures: string[] = [];
      const application = applicationOf(state, [], failures);
      answered(state, application, OFFERED);
      state.button.click();
      answered(state, application, { status: "failed", reason: "Apply failed: parameters changed.",
        failure: { message: "Apply failed: parameters changed.", nextAction: { kind: "tune", label: "Tune controller" } } });
      assert.deepStrictEqual(failures, ["Apply failed: parameters changed."]);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", false],
        "a refusal withdraws nothing: the offers are the engine's, and the engine refuses the next press with its reason");
    } finally { restore(state); }
  });

  test("the end of an Apply puts Restore previous settings in the position and makes Keep in project the primary action the engine names", () => {
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, OFFERED);
      state.button.click();
      answered(state, application, APPLIED);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Restore previous settings", false]);
      assert.deepStrictEqual([state.keep.hidden, state.keep.disabled, state.keep.classList.contains("trust-button--primary"),
        state.button.classList.contains("trust-button--primary")], [false, false, true, false], "Keep in project is the one primary action");
      assert.strictEqual(state.notice.textContent, "Applied: Kp 3.", "the notice is the engine's reason");
    } finally { restore(state); }
  });

  test("keeps Restore in the position through edits, until a Validate answer offers Apply for a new candidate", () => {
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, OFFERED);
      answered(state, application, APPLIED);
      answered(state, application, { status: "edited", stageEvidence: { validationPassed: false }, validationInvalidated: true });
      answered(state, application, { status: "candidate", stageEvidence: { validationPassed: false } });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Restore previous settings", false]);
      answered(state, application, OFFERED);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", false],
        "a candidate that passed validation after the Apply may be applied (the Validate page specification)");
      assert.strictEqual(state.keep.hidden, false, "the values the Apply left are still the ones Keep in project would keep");
      assert.deepStrictEqual([state.button.classList.contains("trust-button--primary"), state.keep.classList.contains("trust-button--primary")], [true, false],
        "Apply is the one primary action the engine names while it offers Apply");
    } finally { restore(state); }
  });

  test("the position holds the action the engine names: Restore while a checkpoint waits, though an earlier answer offered Apply", () => {
    // B6.4, B7: a reopened loop whose Apply's checkpoint waits for its Restore offers Restore and no Apply, and names Restore.
    const state = setup();
    try {
      const posts: string[] = [];
      const application = applicationOf(state, posts);
      answered(state, application, OFFERED);
      answered(state, application, { status: "opened", applyObservationResumable: false,
        engineOutcome: { applyAvailable: false, restoreAvailable: true, keepAvailable: false, primaryAction: "restore" } });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled, state.button.classList.contains("trust-button--primary")],
        ["Restore previous settings", false, true]);
      state.button.click();
      assert.deepStrictEqual(posts, ["control-studio.application.restore"], "the press asks the engine for the Restore review");
      assert.strictEqual(state.button.textContent, "Restoring…");
    } finally { restore(state); }
  });

  test("a failure's next action Restore asks the engine for the Restore review through the Apply area, which shows the answer", () => {
    // A11.2, B6.4: the next action is the engine's; its control sends the Restore request, and the refusal comes back to this area.
    const state = setup();
    try {
      const posts: string[] = [];
      const failures: string[] = [];
      const application = applicationOf(state, posts, failures);
      application.restore();
      assert.deepStrictEqual(posts, ["control-studio.application.restore"]);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Restoring…", true]);
      application.restore();
      assert.deepStrictEqual(posts, ["control-studio.application.restore"], "a second press waits for the engine's answer to the first");
      answered(state, application, { status: "failed", reason: "Restore previous settings is not available.",
        failure: { message: "Restore previous settings is not available.", nextAction: { kind: "none", label: "" } } });
      assert.deepStrictEqual(failures, ["Restore previous settings is not available."]);
      assert.strictEqual(state.button.textContent, "Apply", "the position is the engine's: it offered nothing here");
    } finally { restore(state); }
  });

  test("a confirmed Restore offers neither Restore nor Keep in project, and no Apply until a validation passes", () => {
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, OFFERED);
      answered(state, application, APPLIED);
      state.button.click();
      answered(state, application, { status: "commissioned", applicationAction: "restore", reason: "Restored.",
        engineOutcome: { applyAvailable: false, restoreAvailable: false, keepAvailable: false } });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", true]);
      assert.strictEqual(state.keep.hidden, true, "restored values are the previous ones; there is nothing new to keep");
      assert.strictEqual(state.notice.textContent, "Restored.");
      answered(state, application, OFFERED);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", false]);
    } finally { restore(state); }
  });

  test("a refused Restore leaves Restore offered, and a failed Apply shows the failure the engine gives", () => {
    const state = setup();
    try {
      const failures: string[] = [];
      const application = applicationOf(state, [], failures);
      answered(state, application, OFFERED);
      answered(state, application, APPLIED);
      state.button.click();
      answered(state, application, { status: "failed", reason: "The runtime changed since the review.",
        failure: { message: "The runtime changed since the review.", nextAction: { kind: "none", label: "" } } });
      assert.deepStrictEqual(failures, ["The runtime changed since the review."]);
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Restore previous settings", false],
        "a transient refusal does not take the engine's Restore away");
    } finally { restore(state); }
  });

  test("offers Keep in project after Apply, shows its progress, and takes the engine's offers from its end (Specification 35 B6.5)", () => {
    const state = setup();
    try {
      const posts: string[] = [];
      const failures: string[] = [];
      const application = applicationOf(state, posts, failures);
      answered(state, application, OFFERED);
      assert.strictEqual(state.keep.hidden, true, "nothing is applied yet, so there is nothing to keep");
      state.button.click();
      answered(state, application, APPLIED);
      state.keep.click();
      assert.deepStrictEqual(posts, ["control-studio.application.apply", "control-studio.application.keep"]);
      assert.deepStrictEqual([state.keep.textContent, state.keep.disabled, state.button.disabled], ["Keeping…", true, true]);
      answered(state, application, { status: "failed", reason: "Not kept: the project changed while preparing this edit.",
        failure: { message: "Not kept: the project changed while preparing this edit.", nextAction: { kind: "none", label: "" } } });
      assert.deepStrictEqual([state.keep.hidden, state.keep.disabled, state.keep.textContent], [false, false, "Keep in project"], "a refusal keeps the action");
      assert.deepStrictEqual(failures, ["Not kept: the project changed while preparing this edit."]);
      state.keep.click();
      answered(state, application, { status: "kept", applicationAction: "keep", reason: "Kept in the project: ProportionalGain of P1.Parameters.",
        engineOutcome: { restoreAvailable: true, keepAvailable: false, primaryAction: "restore" } });
      assert.strictEqual(state.keep.hidden, true);
      assert.strictEqual(state.notice.textContent, "Kept in the project: ProportionalGain of P1.Parameters.");
      assert.deepStrictEqual([state.button.textContent, state.button.classList.contains("trust-button--primary")], ["Restore previous settings", true],
        "the settings still stand, so the engine names Restore in the position");
    } finally { restore(state); }
  });

  test("an opened loop shows the offers the engine answers: Restore holds the position though a validation passed", () => {
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, OFFERED);
      answered(state, application, { status: "opened", applyObservationResumable: false,
        engineOutcome: { applyAvailable: false, restoreAvailable: true, keepAvailable: true, primaryAction: "keep" } });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled, state.keep.hidden], ["Restore previous settings", false, false]);
    } finally { restore(state); }
  });

  test("an opened loop with a passed validation and nothing to restore offers Apply", () => {
    const state = setup();
    try {
      const application = applicationOf(state);
      answered(state, application, { status: "opened", applyObservationResumable: false,
        engineOutcome: { applyAvailable: true, restoreAvailable: false, keepAvailable: false, primaryAction: "apply" } });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled, state.keep.hidden], ["Apply", false, true]);
      answered(state, application, { status: "opened", applyObservationResumable: false,
        engineOutcome: { applyAvailable: false, restoreAvailable: false, keepAvailable: false } });
      assert.strictEqual(state.button.disabled, true, "a reopened loop takes what the engine says now, not what it said before");
    } finally { restore(state); }
  });

  test("a stage block, such as a reset or a running experiment, holds Apply and Restore whatever the engine offers", () => {
    const state = setup();
    try {
      let blocked = true;
      const application = new ThreePageApplication(state.root, () => {}, () => false, () => {}, () => {}, () => blocked);
      answered(state, application, OFFERED);
      assert.strictEqual(state.button.disabled, true);
      blocked = false;
      application.synchronize();
      assert.strictEqual(state.button.disabled, false);
    } finally { restore(state); }
  });
});
