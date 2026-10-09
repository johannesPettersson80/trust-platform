import * as assert from "assert";
import { object, Webview } from "./hmiBuilderWebview";

suite("HMI Builder editor: display limits", function () {
  this.timeout(15000);
  test("valid ordered limits change together, invalid limits leave the page unchanged, and clearing removes them", async () => {
    const view = new Webview();
    try {
      await view.loaded({
        title: "Level",
        pipe: [],
        object: [object("level", "gauge", [40, 40], [160, 160], { min: 0, max: 100 })],
      });
      view.drag(view.layer(), [80, 80], [80, 80]);
      await view.until("selected gauge", () => view.query('input[name="id"]')?.value === "level");
      const low = view.query('input[name="limit-low"]');
      assert.ok(low, "the inspector offers display alarm limits");
      view.type(low, "90");
      view.type(view.query('input[name="limit-high"]'), "10");
      view.query('[data-action="apply-limits"]').click();
      await new Promise((resolve) => setTimeout(resolve, 50));
      assert.strictEqual(view.edits().length, 0, "reversed thresholds never edit the document");
      assert.match(view.query('[data-part="display-limits"] [role="alert"]').textContent, /ordered/i);
      view.type(low, "10");
      view.type(view.query('input[name="limit-high"]'), "90");
      view.query('[data-action="apply-limits"]').click();
      const edited = await view.nextEdit(0);
      assert.deepStrictEqual(edited.object[0].limits, { low: 10, high: 90 });
      assert.strictEqual(view.edits().length, 1, "all thresholds are one text edit");
      view.send({ type: "page", model: edited });
      await view.until("the normal band", () => view.query('[data-object="level"] [data-part="normal-range"]'));
      view.type(view.query('input[name="limit-low"]'), "");
      view.type(view.query('input[name="limit-high"]'), "");
      view.query('[data-action="apply-limits"]').click();
      assert.strictEqual((await view.nextEdit(1)).object[0].limits, undefined);
    } finally {
      view.window.close();
    }
  });
});
