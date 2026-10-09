import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { object, PAGE, Webview } from "./hmiBuilderWebview";

// Specification 39, §3.3 item 8: the Layers panel lists the page's objects front to back; a click
// selects, dragging reorders, the eye hides an object in the editor only, and the lock, saved as
// `locked = true`, stops it being selected or moved on the canvas.

const THREE: HmiPageModel = { ...PAGE, object: [...PAGE.object, object("wide", "value", [100, 250], [400, 96])] };

const selected = (view: Webview, id: string) => view.query(`[data-selection="${id}"]`);
const rows = (view: Webview): string[] =>
  Array.from(view.document.querySelectorAll("[data-layer]"), (row: any) => row.getAttribute("data-layer"));

async function layers(view: Webview, page: HmiPageModel = THREE): Promise<void> {
  await view.loaded(page);
  (await view.until("the Layers tab", () => view.query('[data-left-tab="layers"]'))).click();
  await view.until("the Layers panel", () => rows(view).length === page.object.length);
}

suite("HMI Builder editor: the Layers panel", function () {
  this.timeout(20000);

  test("the objects are listed front to back, and a click on one selects it", async () => {
    const view = new Webview();
    await layers(view);
    assert.deepStrictEqual(rows(view), ["wide", "level", "speed"]);
    view.query('[data-layer="level"]').click();
    await view.until("level selected", () => selected(view, "level") && !selected(view, "speed"));
  });

  test("dragging a row onto another puts its object in front of that one", async () => {
    const view = new Webview();
    await layers(view);
    view.mouse("mousedown", view.query('[data-layer="speed"]'), 0, 0);
    view.mouse("mouseup", view.query('[data-layer="wide"]'), 0, 0);
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.map((candidate) => candidate.id),
      ["level", "wide", "speed"],
    );
  });

  test("the eye hides an object in the editor only", async () => {
    const view = new Webview();
    await layers(view);
    view.query('[data-layer="level"] [data-layer-eye]').click();
    await view.until("level gone from the canvas", () => !view.query('svg [data-object="level"]'));
    assert.ok(view.query('svg [data-object="speed"]'), "the others stay");
    assert.strictEqual(view.edits().length, 0, "nothing is written to the page");
    view.query('[data-layer="level"] [data-layer-eye]').click();
    await view.until("level back", () => view.query('svg [data-object="level"]'));
  });

  test("the lock is saved and stops the object being selected on the canvas", async () => {
    const view = new Webview();
    await layers(view);
    view.query('[data-layer="speed"] [data-layer-lock]').click();
    const edit = await view.nextEdit(0);
    assert.strictEqual(edit.object.find((candidate) => candidate.id === "speed")?.locked, true);
    view.send({ type: "page", model: edit });
    view.key("Escape");
    view.mouse("mousedown", view.layer(), 60, 60);
    view.mouse("mouseup", view.window, 60, 60);
    view.key("a", { ctrlKey: true });
    await view.until("Ctrl+A selects the others", () => selected(view, "level") && selected(view, "wide"));
    assert.ok(!selected(view, "speed"), "the locked object is not selected");
  });
});
