import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { CATALOGUE, object, PAGE, SCREEN, Webview } from "./hmiBuilderWebview";

// Specification 39, §4 item 5: in Design mode the editor previews the page on smaller screens,
// as the runtime lays it out there, read-only.

const WITH_PANEL: HmiPageModel = {
  ...PAGE,
  object: [...PAGE.object, object("box", "panel", [900, 40], [340, 200], { label: "Pump" })],
};

suite("HMI Builder editor: screen size previews", function () {
  this.timeout(10000);
  const rendered = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150));
  const boxAt = (view: Webview): string | null =>
    view.query('svg [data-object="box"]')?.getAttribute("transform") ?? null;

  test("a smaller screen previews the page as the runtime lays it out, read-only, until the design size again", async () => {
    const view = new Webview();
    await view.loaded(WITH_PANEL);
    assert.strictEqual(boxAt(view), "translate(900 40)");
    const sizes = view.query('[data-action="preview-size"]');
    assert.ok(sizes, "the top bar offers the screen sizes");
    const offered = [...sizes.querySelectorAll("option")].map((option: any) => option.textContent);
    assert.deepStrictEqual(offered, [
      "Design size",
      "1920 × 1080",
      "1280 × 800",
      "1024 × 768",
      "Tablet landscape 1180 × 820",
      "Tablet portrait 820 × 1180",
    ]);
    view.choose(sizes, "1024x768");
    await rendered();
    // 1024 px less the 40 px margin less the panel's 340 px.
    assert.strictEqual(boxAt(view), "translate(644 40)");
    assert.strictEqual(view.query('[data-part="canvas-page"]').style.width, "1024px");
    view.drag(view.layer(), [960, 80], [700, 300]);
    await rendered();
    assert.strictEqual(view.edits().length, 0, "the preview edits nothing");
    view.choose(view.query('[data-action="preview-size"]'), "design");
    await rendered();
    assert.strictEqual(boxAt(view), "translate(900 40)");
  });

  test("a panel's inspector sets how it rearranges its objects on a smaller screen", async () => {
    const view = new Webview();
    await view.loaded(WITH_PANEL);
    view.drag(view.layer(), [1200, 220], [1200, 220]);
    await view.until("the panel selected", () => view.query('[data-selection="box"]'));
    const arrange = view.query('select[name="arrange"]');
    assert.ok(arrange, "the inspector offers the arrangement");
    assert.deepStrictEqual(
      [...arrange.querySelectorAll("option")].map((option: any) => option.value),
      ["", "row", "column", "grid"],
    );
    const panel = (): any =>
      view
        .edits()
        .pop()
        ?.object.find((item) => item.id === "box");
    view.choose(arrange, "row");
    await rendered();
    assert.strictEqual(panel()?.arrange, "row");
    view.send({
      type: "page",
      model: {
        ...WITH_PANEL,
        object: WITH_PANEL.object.map((item) => (item.id === "box" ? { ...item, arrange: "row" as const } : item)),
      },
    });
    await rendered();
    const gap = view.query('input[name="gap"]');
    assert.ok(gap, "an arranged panel takes its gap");
    view.type(gap, "12");
    await rendered();
    assert.deepStrictEqual([panel()?.arrange, panel()?.gap], ["row", 12]);
    view.choose(view.query('select[name="arrange"]'), "");
    await rendered();
    assert.deepStrictEqual(
      [panel()?.arrange, panel()?.gap],
      [undefined, undefined],
      "as designed again, without a gap",
    );
  });

  test("the page area is the screen less its top bar and side bar", async () => {
    const view = new Webview();
    await view.ready();
    view.send({ type: "catalogue", catalogue: CATALOGUE });
    view.send({ type: "screen", screen: { ...SCREEN, topBar: true, navigation: "side" } });
    view.send({ type: "page", model: WITH_PANEL });
    await view.until("the page", () => view.query('svg [data-object="box"]'));
    const sizes = view.query('[data-action="preview-size"]');
    assert.ok(sizes, "the top bar offers the screen sizes");
    view.choose(sizes, "1280x800");
    await rendered();
    // 1280 less the 232 px side bar: the panel moves left to 1048 - 40 - 340.
    assert.strictEqual(boxAt(view), "translate(668 40)");
    assert.strictEqual(view.query('[data-part="canvas-page"]').style.width, "1048px");
  });
});
