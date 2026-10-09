import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { Webview } from "./hmiBuilderWebview";

// Specification 39, §21.3 item 6: the canvas's shortcuts (arrows, Delete, Enter, …) act only while
// the canvas has the focus; while a button, a field or a menu has it, the keys belong to it.

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 100));
const placeOf = (page: HmiPageModel | undefined, id: string): [number, number] | undefined =>
  page?.object.find((object) => object.id === id)?.at;

/** A key pressed where the focus is, as a user presses it. */
function press(view: Webview, target: any, key: string): void {
  target.dispatchEvent(new view.window.KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

suite("HMI Builder editor: keyboard focus (§21.3 item 6)", function () {
  this.timeout(20000);

  test("an arrow key moves the selected object while the canvas has the focus, and does nothing while a toolbar button has it", async () => {
    const view = new Webview();
    await view.loaded();
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("speed selected", () => view.query('[data-selection="speed"]'));

    const zoomIn = view.query('.hb-topbar [title="Zoom in"]');
    zoomIn.focus();
    assert.strictEqual(view.document.activeElement, zoomIn, "the toolbar button has the focus");
    for (const key of ["ArrowRight", "Delete", "Enter"]) press(view, zoomIn, key);
    await settle();
    assert.deepStrictEqual(view.edits(), [], "the keys belong to the button: nothing is moved or deleted");
    assert.strictEqual(view.document.activeElement, zoomIn, "Enter stays with the button, not the inspector");
    assert.ok(view.query('[data-selection="speed"]'), "the selection stays");

    // A press on the canvas gives it the keys.
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("the button gave up the focus", () => view.document.activeElement !== zoomIn);
    press(view, view.document.activeElement, "ArrowRight");
    assert.deepStrictEqual(placeOf(await view.nextEdit(0), "speed"), [48, 40], "one grid step to the right");
  });
});
