import * as assert from "assert";
import { object, Webview } from "./hmiBuilderWebview";

suite("HMI Builder editor: trend scales", function () {
  this.timeout(15000);
  test("a pen's fixed limits are one document edit and Automatic removes the override", async () => {
    const view = new Webview();
    try {
      await view.loaded({
        title: "Trends",
        pipe: [],
        object: [
          object("trend", "trend", [40, 40], [480, 300], {
            bind: { Temperature: { path: "Main.Temperature", direction: "read" } },
          }),
        ],
      });
      view.drag(view.layer(), [80, 80], [80, 80]);
      await view.until("the selected trend", () => view.query('input[name="id"]')?.value === "trend");
      const scale = view.query('select[data-pen-scale="Temperature"]');
      assert.ok(scale, "the trend inspector offers a scale for each pen");
      view.choose(scale, "fixed");
      const low = await view.until("the fixed limits", () => view.query('input[name="pen-min"]'));
      view.type(low, "-20");
      view.type(view.query('input[name="pen-max"]'), "120");
      view.query('[data-action="apply-pen-scale"]').click();
      const edited = await view.nextEdit(0);
      assert.deepStrictEqual((edited.object[0].bind.Temperature as any).range, [-20, 120]);
      assert.strictEqual(view.edits().length, 1, "both limits change atomically");
      view.send({ type: "page", model: edited });
      await new Promise((resolve) => setTimeout(resolve, 50));
      view.choose(view.query('select[data-pen-scale="Temperature"]'), "auto");
      assert.ok(!(await view.nextEdit(1)).object[0].bind.Temperature.range);
    } finally {
      view.window.close();
    }
  });
});
