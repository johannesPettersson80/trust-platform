import * as assert from "assert";
import type { HostToWebview } from "../../hmiBuilder/editor/messages";
import { CATALOGUE, object, Webview } from "./hmiBuilderWebview";

suite("HMI Builder editor: inherited formats", function () {
  this.timeout(15000);
  test("project format changes repaint the preview without editing its authored properties", async () => {
    const view = new Webview();
    try {
      await view.loaded({
        title: "Level",
        pipe: [],
        object: [
          object("level", "value", [40, 40], [240, 96], { bind: { value: { path: "Main.Level", direction: "read" } } }),
        ],
      });
      const send = (unit: string, decimals: number) =>
        view.send({
          type: "catalogue",
          catalogue: CATALOGUE,
          formats: { "main.level": { unit, decimals } },
        } as HostToWebview);
      send("m", 2);
      await new Promise((resolve) => setTimeout(resolve, 80));
      let text = view.query('[data-object="level"]').textContent as string;
      assert.match(text, /—m/, "the preview uses the inherited format");
      send("cm", 1);
      await new Promise((resolve) => setTimeout(resolve, 80));
      text = view.query('[data-object="level"]').textContent as string;
      assert.match(text, /—cm/, "another page's unsaved edit refreshes the preview");
      assert.strictEqual(view.edits().length, 0, "inheritance never persists a local override");
    } finally {
      view.window.close();
    }
  });
});
