import * as assert from "assert";
import { ThreePageApplication } from "../../controlStudio/webview/threePageApplication";

const { JSDOM } = require("jsdom");

function setup() {
  const dom = new JSDOM(`<main data-studio-three-page>
    <section data-studio-page="verify"><div data-studio-application>
      <p data-studio-message="verify"></p><button data-engineer-apply>Apply</button>
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
  return { dom, root, button: root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!, notice: root.querySelector<HTMLElement>("[data-studio-message=verify]")!, previous };
}

function restore(state: ReturnType<typeof setup>): void {
  Object.assign(globalThis, state.previous);
  state.dom.window.close();
}

suite("Control Studio three-page application projection", () => {
  test("does not enable Apply when verification evidence is explicitly not passed", () => {
    const state = setup();
    try {
      const application = new ThreePageApplication(state.root, () => {}, () => false, message => { state.notice.textContent = message; });
      application.receive({ status: "verified", stageEvidence: { validationPassed: false } });
      assert.strictEqual(state.button.disabled, true);
    } finally { restore(state); }
  });

  test("shows Apply's progress while it waits for the process to settle, and keeps Apply available after a cancelled wait", () => {
    const state = setup();
    try {
      const posts: string[] = [];
      const application = new ThreePageApplication(state.root, kind => { posts.push(kind); }, () => false, message => { state.notice.textContent = message; });
      application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      state.button.click();
      assert.deepStrictEqual(posts, ["control-studio.application.apply"]);
      application.receive({ status: "running", activity: "apply-settling", applicationAction: "apply",
        reason: "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…" });
      assert.strictEqual(state.notice.textContent, "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…");
      assert.strictEqual(state.button.textContent, "Applying…");
      application.receive({ status: "failed", applicationAction: "apply", failure: { nextAction: "reapply" },
        reason: "Apply cancelled while waiting for the process to settle; nothing was written." });
      assert.deepStrictEqual([state.button.textContent, state.button.disabled], ["Apply", false], "the validated candidate stays applicable");
    } finally { restore(state); }
  });

  test("keeps Restore available through edits until a new candidate passes", () => {
    const state = setup();
    try {
      const application = new ThreePageApplication(state.root, () => {}, () => false, message => { state.notice.textContent = message; });
      application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      application.receive({ status: "commissioned", applicationAction: "apply" });
      assert.strictEqual(state.button.textContent, "Restore previous settings");
      assert.strictEqual(state.button.disabled, false);
      application.receive({ status: "edited" });
      assert.strictEqual(state.button.textContent, "Restore previous settings");
      assert.strictEqual(state.button.disabled, false);
      application.receive({ status: "candidate" });
      assert.strictEqual(state.button.textContent, "Restore previous settings");
      assert.strictEqual(state.button.disabled, false);
      application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      assert.strictEqual(state.button.textContent, "Apply");
      assert.strictEqual(state.button.disabled, false);
    } finally { restore(state); }
  });

  test("projects explicit observed application availability and keeps historical receipts unavailable", () => {
    const state = setup();
    try {
      const application = new ThreePageApplication(state.root, () => {}, () => false, message => { state.notice.textContent = message; });
      application.receive({ status: "applied", applicationAvailability: "restore", applicationAction: "apply" });
      assert.strictEqual(state.button.textContent, "Restore previous settings");
      assert.strictEqual(state.button.disabled, false);
      application.receive({ status: "unavailable", applicationAvailability: "unavailable", reason: "Current runtime receipt is unavailable." });
      assert.strictEqual(state.button.disabled, true);
      assert.strictEqual(state.notice.textContent, "Current runtime receipt is unavailable.");

      const historical = setup();
      try {
        const restored = new ThreePageApplication(historical.root, () => {}, () => false, () => {});
        restored.receive({ status: "restored", applicationAction: "restore" });
        assert.strictEqual(historical.button.disabled, true, "A historical receipt alone must not authorize a new application");
      } finally { restore(historical); }
    } finally { restore(state); }
  });

  test("keeps Apply available when a refusal says Apply again", () => {
    const state = setup();
    try {
      const application = new ThreePageApplication(state.root, () => {}, () => false, message => { state.notice.textContent = message; });
      application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      state.button.click();
      application.receive({ status: "failed", failure: { cause: "settings", nextAction: "reapply" },
        reason: "Apply failed: P1.ProcessValue is 20.56 °C, but the settings were validated from 20 °C ± 0.029 °C. Wait for the process to settle there before applying. Next: Apply again." });
      assert.strictEqual(state.button.textContent, "Apply");
      assert.strictEqual(state.button.disabled, false, "the validated settings stay applicable once the process has settled");
      assert.match(state.notice.textContent ?? "", /Wait for the process to settle/);
      state.button.click();
      application.receive({ status: "failed", failure: { cause: "settings", nextAction: "retune" }, reason: "Apply failed: parameters changed. Next: Tune again." });
      assert.strictEqual(state.button.disabled, true, "any other refusal withdraws the validated application");
    } finally { restore(state); }
  });

  test("failed Restore retains its reason and requires a fresh reviewed Restore", () => {
    const state = setup();
    try {
      const application = new ThreePageApplication(state.root, () => {}, () => false, message => { state.notice.textContent = message; });
      application.receive({ status: "applied", applicationAvailability: "restore", applicationAction: "apply" });
      state.button.click();
      application.receive({ status: "failed", applicationAction: "restore", reason: "Restore was rejected by the current runtime." });
      assert.strictEqual(state.button.disabled, true);
      assert.strictEqual(state.notice.textContent, "Restore was rejected by the current runtime.");
      application.receive({ status: "review", applicationAvailability: "restore", applicationAction: "restore" });
      assert.strictEqual(state.button.textContent, "Restore previous settings");
      assert.strictEqual(state.button.disabled, false);
    } finally { restore(state); }
  });
});
