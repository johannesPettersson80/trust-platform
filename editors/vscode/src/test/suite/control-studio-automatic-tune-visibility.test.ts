import * as assert from "assert";
import { initializeAutomaticTuneControls } from "../../controlStudio/webview/automaticTuneControls";

const { JSDOM } = require("jsdom");

suite("Control Studio Automatic Tune visibility", () => {
  test("opens the plan through a visible workspace surface from a configured loop", () => {
    const dom = new JSDOM(`
      <main class="studio" data-control-studio data-guided="true" data-manual-workspace="false">
        <button data-control-studio-automatic-tune>Automatic Tune</button>
        <section class="workspace" style="display:none">
          <section class="auto-plan" data-auto-tune-plan hidden>
            <button data-auto-tune-close>Close</button>
            <button data-auto-tune-approve>Confirm and run</button>
          </section>
        </section>
      </main>`);
    const doc = dom.window.document as Document;
    const previousDocument = (globalThis as { document?: Document }).document;
    (globalThis as { document?: Document }).document = dom.window.document;
    try {
      initializeAutomaticTuneControls({ post: () => undefined, showError: () => undefined });
      doc.querySelector<HTMLButtonElement>("[data-control-studio-automatic-tune]")?.click();

      const plan = doc.querySelector<HTMLElement>("[data-auto-tune-plan]")!;
      assert.strictEqual(plan.hidden, false);
      for (let parent: HTMLElement | null = plan; parent; parent = parent.parentElement) {
        assert.strictEqual(parent.hidden, false, "Opened plan must not have a hidden ancestor");
        assert.notStrictEqual(dom.window.getComputedStyle(parent).display, "none", "Opened plan must not have a display:none ancestor");
      }
      doc.querySelector<HTMLButtonElement>("[data-auto-tune-close]")?.click();
      assert.strictEqual(plan.hidden, true);
      assert.strictEqual(plan.closest(".workspace") !== null, true, "Closing must restore the plan to its original workspace");
      doc.querySelector<HTMLButtonElement>("[data-control-studio-automatic-tune]")?.click();
      doc.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      assert.strictEqual(plan.hidden, true, "Escape must close the visible plan");
      assert.strictEqual(doc.activeElement?.getAttribute("data-control-studio-automatic-tune"), "",
        "closing must return focus to the launcher");
    } finally {
      if (previousDocument) (globalThis as { document?: Document }).document = previousDocument;
      else delete (globalThis as { document?: Document }).document;
    }
  });
});
