import * as assert from "assert";
import { initializeThreePageStateDesign } from "../../controlStudio/webview/threePageStateDesign";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

suite("Control Studio state design preferences", () => {
  test("shows declared Q and R defaults while preserving blank drafts as explicit edits", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main><section data-studio-state-design></section></main>');
    // The loop's declared design is the engine's text, as the open answer gives it (C4); the form shows it until the study holds a draft.
    setOpenedOffers({ stateDeclaredText: { stateWeights: "[[2]]", inputWeights: "[[3]]" } });
    try {
      const root = dom.window.document.querySelector("main")!;
      const editor = initializeThreePageStateDesign(root, () => undefined) as any;
      editor.setFields([{ field: "stateWeights", label: "State weights (Q)" }, { field: "inputWeights", label: "Input weights (R)" }]);
      assert.strictEqual(root.querySelector('[data-state-design-field="stateWeights"]').value, "[[2]]");
      assert.strictEqual(root.querySelector('[data-state-design-field="inputWeights"]').value, "[[3]]");
      const input = root.querySelector('[data-state-design-field="inputWeights"]'); input.value = ""; input.dispatchEvent(new dom.window.Event("input"));
      assert.strictEqual(editor.draft().inputWeights, "", "an explicit blank remains a draft and does not silently restore the declared default");
    } finally { setOpenedOffers({}); dom.window.close(); }
  });
});
