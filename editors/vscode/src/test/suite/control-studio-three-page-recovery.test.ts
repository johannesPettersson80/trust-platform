import * as assert from "assert";
import { ThreePageRecovery } from "../../controlStudio/webview/threePageRecovery";

const { JSDOM } = require("jsdom");

function recoveryApi(): any {
  return { ThreePageRecovery };
}

function setup() {
  const dom = new JSDOM(`<main data-studio-three-page>
    <section data-studio-page="verify">
      <div data-studio-application hidden><p data-studio-message="verify"></p><button data-engineer-apply>Apply</button></div>
    </section>
    <select data-studio-controller><option>PID</option></select><input data-studio-requirements>
    <button data-engineer-stage-tab="model">Identify</button><button data-engineer-stage-tab="tune">Tune</button><button data-engineer-stage-tab="verify">Validate</button>
    <button data-engineer-find>Identify plant</button><button data-engineer-tune>Tune controller</button><button data-engineer-verify>Validate controller</button>
  </main>`);
  const posts: Array<{ kind: string; payload?: Record<string, unknown> }> = [];
  const types = Object.fromEntries(["HTMLButtonElement", "HTMLSelectElement", "HTMLInputElement", "HTMLTextAreaElement"].map(key => [key, dom.window[key]]));
  const previous = { ...Object.fromEntries(Object.keys(types).map(key => [key, (globalThis as any)[key]])), document: (globalThis as any).document, window: (globalThis as any).window };
  Object.assign(globalThis, { ...types, document: dom.window.document, window: dom.window });
  return { dom, posts, root: dom.window.document.querySelector("[data-studio-three-page]") as HTMLElement, previous };
}

function restore(state: ReturnType<typeof setup>): void {
  Object.assign(globalThis, state.previous);
  state.dom.window.close();
}

suite("Control Studio three-page recovery projection", () => {
  test("keeps completed commissioning history outside interrupted-operation recovery", () => {
    const state = setup();
    try {
      const recovery = new ThreePageRecovery(state.root, () => {}, () => false);
      recovery.receive({ status: "restored", commissioningCheckpoint: true, commissioning: { status: "passed", recoveryConfirmed: true } });
      assert.strictEqual(recovery.active, false, "A completed receipt is observed before Restore, not recovered as an interrupted operation");
    } finally { restore(state); }
  });
  test("projects one primary cancellation action and locks dependent edits", () => {
    const api = recoveryApi();
    assert.strictEqual(typeof api.ThreePageRecovery, "function");
    const state = setup();
    try {
      const recovery = new api.ThreePageRecovery(state.root, (kind: string, payload?: Record<string, unknown>) => state.posts.push({ kind, payload }), () => false);
      recovery.receive({ status: "recovery-required", cancellationUnconfirmed: true,
        reason: "Reconnect the original simulator and confirm cancellation." });
      const action = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
      assert.strictEqual(action.textContent, "Confirm cancellation");
      assert.strictEqual((state.root.querySelector("[data-studio-application]") as HTMLElement).hidden, false);
      assert.strictEqual(action.dataset.recoveryAction, "cancel");
      assert.ok(action.classList.contains("trust-button--primary"));
      assert.strictEqual((state.root.querySelector("[data-studio-controller]") as HTMLSelectElement).disabled, true);
      assert.strictEqual((state.root.querySelector("[data-engineer-tune]") as HTMLButtonElement).disabled, true);
      action.click();
      assert.deepStrictEqual(state.posts, [{ kind: "control-studio.engineer.cancel", payload: undefined }]);
    } finally { restore(state); }
  });

  test("uses Retry recovery for pending Manual application and never repeats Apply", () => {
    const api = recoveryApi();
    assert.strictEqual(typeof api.ThreePageRecovery, "function");
    const state = setup();
    try {
      const recovery = new api.ThreePageRecovery(state.root, (kind: string, payload?: Record<string, unknown>) => state.posts.push({ kind, payload }), () => false);
      recovery.receive({ status: "recovery-required", retryManualRecovery: true,
        reason: "The retained Manual application needs reconciliation." });
      const action = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
      assert.strictEqual(action.textContent, "Retry recovery");
      assert.strictEqual(action.dataset.recoveryAction, "retry-recovery");
      action.click(); action.click();
      assert.deepStrictEqual(state.posts, [{ kind: "control-studio.engineer.retry-recovery", payload: undefined }]);
    } finally { restore(state); }
  });

  test("offers rollback review separately from explicit restarted-runtime review", () => {
    const api = recoveryApi();
    assert.strictEqual(typeof api.ThreePageRecovery, "function");
    const state = setup();
    try {
      const recovery = new api.ThreePageRecovery(state.root, (kind: string, payload?: Record<string, unknown>) => state.posts.push({ kind, payload }), () => false);
      recovery.receive({ status: "recovery-required", commissioningCheckpoint: true,
        reason: "Review rollback before starting new work." });
      let action = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
      assert.strictEqual(action.textContent, "Review restore");
      assert.strictEqual(action.dataset.recoveryAction, "rollback");
      action.click();
      recovery.receive({ status: "review", rollback: true,
        reason: "Review the previous settings before restoring them." });
      action = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
      assert.strictEqual(action.textContent, "Restore previous settings");
      assert.strictEqual(action.dataset.recoveryAction, "confirm-rollback");
      action.click();
      assert.deepStrictEqual(state.posts.map(post => post.kind), [
        "control-studio.engineer.rollback", "control-studio.engineer.confirm",
      ]);

      recovery.receive({ status: "failed", replacementRuntimeAvailable: true,
        reason: "The original runtime ended before the operation was confirmed." });
      action = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
      assert.strictEqual(action.textContent, "Review restarted runtime");
      assert.strictEqual(action.dataset.recoveryAction, "review-runtime");
      action.click();
      recovery.receive({ status: "review", runtimeReplacementReview: true,
        reason: "Confirm archiving the unresolved operation from the previous runtime." });
      action = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
      assert.strictEqual(action.textContent, "Confirm new runtime");
      assert.strictEqual(action.dataset.recoveryAction, "confirm-runtime");
      action.click();
      assert.deepStrictEqual(state.posts.map(post => post.kind), [
        "control-studio.engineer.rollback", "control-studio.engineer.confirm",
        "control-studio.engineer.review-runtime", "control-studio.engineer.confirm",
      ]);
    } finally { restore(state); }
  });

  test("clears the temporary recovery action only after reconciliation", () => {
    const api = recoveryApi();
    assert.strictEqual(typeof api.ThreePageRecovery, "function");
    const state = setup();
    try {
      const recovery = new api.ThreePageRecovery(state.root, () => {}, () => false);
      recovery.receive({ status: "recovery-required", retryManualRecovery: true, reason: "reconcile" });
      assert.ok(state.root.querySelector("[data-studio-recovery-action]"));
      recovery.receive({ status: "recovery-required", retryManualRecovery: true, reason: "still unresolved" });
      assert.ok(state.root.querySelector("[data-studio-recovery-action]"));
      recovery.receive({ status: "failed", recoveryResolved: true, reason: "No operation was replayed." });
      assert.strictEqual(state.root.querySelector("[data-studio-recovery-action]"), null);
      assert.strictEqual((state.root.querySelector("[data-studio-controller]") as HTMLSelectElement).disabled, false);
    } finally { restore(state); }
  });

  test("does not treat a normal Restore application review as recovery confirmation", () => {
    const state = setup();
    try {
      const recovery = new ThreePageRecovery(state.root, (kind, payload) => state.posts.push({ kind, payload }), () => false);
      recovery.receive({ status: "review", rollback: true, applicationAction: "restore", reason: "Normal reviewed restore." });
      assert.strictEqual(state.root.querySelector("[data-studio-recovery-action]"), null);
      assert.strictEqual((state.root.querySelector("[data-engineer-apply]") as HTMLButtonElement).hidden, false);
      assert.strictEqual((state.root.querySelector("[data-studio-controller]") as HTMLSelectElement).disabled, false);
      assert.deepStrictEqual(state.posts, []);
    } finally { restore(state); }
  });

  test("keeps cancellation recovery visible for failed and restored replies", () => {
    for (const status of ["failed", "restored"]) {
      const state = setup();
      try {
        const recovery = new ThreePageRecovery(state.root, () => {}, () => false);
        recovery.receive({ status, cancellationUnconfirmed: true, reason: "Cancellation is still unresolved." });
        assert.strictEqual(state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")?.dataset.recoveryAction, "cancel");
      } finally { restore(state); }
    }
  });

  test("synchronize preserves locks while leaving stage navigation enabled", () => {
    const state = setup();
    try {
      const recovery = new ThreePageRecovery(state.root, () => {}, () => false);
      recovery.receive({ status: "recovery-required", retryManualRecovery: true, reason: "Reconcile first." });
      recovery.synchronize();
      assert.strictEqual((state.root.querySelector("[data-studio-controller]") as HTMLSelectElement).disabled, true);
      for (const tab of state.root.querySelectorAll<HTMLButtonElement>("[data-engineer-stage-tab]")) assert.strictEqual(tab.disabled, false);
    } finally { restore(state); }
  });

  test("restores the same retry action after a generic failed or unavailable recovery reply", () => {
    for (const status of ["failed", "unavailable"]) {
      const state = setup();
      try {
        const recovery = new ThreePageRecovery(state.root, () => {}, () => false);
        recovery.receive({ status: "recovery-required", retryManualRecovery: true, reason: "Reconcile first." });
        const first = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
        first.click();
        recovery.receive({ status, reason: "The retry could not be confirmed." });
        const retry = state.root.querySelector<HTMLButtonElement>("[data-studio-recovery-action]")!;
        assert.strictEqual(retry.dataset.recoveryAction, "retry-recovery");
        assert.strictEqual(retry.disabled, false);
        assert.strictEqual(retry.textContent, "Retry recovery");
      } finally { restore(state); }
    }
  });
});
