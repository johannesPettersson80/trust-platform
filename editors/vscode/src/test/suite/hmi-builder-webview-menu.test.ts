import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { object, PAGE, Webview } from "./hmiBuilderWebview";

// Specification 39, §3.3 item 8: the context menu: cut, copy, paste, duplicate, delete, group,
// layer order, align, lock, and on a symbol, open its faceplate.

const menu = (view: Webview) => view.query('[data-part="context-menu"]');
const item = (view: Webview, action: string) => view.query(`[data-part="context-menu"] [data-menu="${action}"]`);

function rightClick(view: Webview, x: number, y: number): void {
  view.mouse("contextmenu", view.layer(), x, y, { button: 2 });
}

suite("HMI Builder editor: the context menu", function () {
  this.timeout(20000);

  test("a right-click on an object selects it and opens its menu; Copy and Delete act on it", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    rightClick(view, 60, 60);
    await view.until("the menu", () => menu(view));
    assert.ok(view.query('[data-selection="speed"]'), "the object is selected");
    for (const action of ["cut", "copy", "paste", "duplicate", "delete", "front", "back", "lock"])
      assert.ok(item(view, action), action);
    item(view, "copy").click();
    await view.until("copied", () => view.posted.some((message) => message.type === "copy"));
    assert.ok(!menu(view), "the menu closes after its action");
    rightClick(view, 60, 60);
    (await view.until("the menu again", () => item(view, "delete"))).click();
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(
      edit.object.map((candidate) => candidate.id),
      ["level"],
    );
  });

  test("with two objects selected the menu groups, aligns and orders them; Lock locks them", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.key("a", { ctrlKey: true });
    await view.until(
      "both selected",
      () => view.query('[data-selection="speed"]') && view.query('[data-selection="level"]'),
    );
    rightClick(view, 60, 60);
    await view.until("the menu", () => menu(view));
    assert.ok(item(view, "group") && item(view, "align-left") && item(view, "forward"), "group, align and order");
    item(view, "align-bottom").click();
    const aligned = await view.nextEdit(0);
    assert.deepStrictEqual(
      aligned.object.map((candidate) => candidate.at[1]),
      [184, 40],
    );
    view.send({ type: "page", model: aligned });
    rightClick(view, 340, 60);
    (await view.until("lock", () => item(view, "lock"))).click();
    const locked = await view.nextEdit(1);
    assert.deepStrictEqual(
      locked.object.map((candidate) => candidate.locked),
      [true, true],
    );
  });

  test("on a symbol with a faceplate, Open faceplate asks the editor's host to open it", async () => {
    const view = new Webview();
    const page: HmiPageModel = {
      ...PAGE,
      object: [object("p1", "pump", [40, 40], [128, 108], { click: { faceplate: "pump", instance: "Main.P101" } })],
    };
    await view.loaded(page);
    rightClick(view, 80, 80);
    (await view.until("open faceplate", () => item(view, "faceplate"))).click();
    await view.until("asked to open", () =>
      view.posted.some((message) => message.type === "openFaceplate" && message.name === "pump"),
    );
  });

  test("near the window's corner the menu opens inward, so all of it shows", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    // jsdom lays nothing out: the menu measures 200 x 300 in a 1024 x 768 window.
    const measure = view.window.HTMLElement.prototype.getBoundingClientRect;
    view.window.HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
      return this.getAttribute("data-part") === "context-menu"
        ? { x: 0, y: 0, left: 0, top: 0, width: 200, height: 300, right: 200, bottom: 300, toJSON: () => ({}) }
        : measure.call(this);
    };
    Object.defineProperty(view.window, "innerWidth", { value: 1024, configurable: true });
    Object.defineProperty(view.window, "innerHeight", { value: 768, configurable: true });
    rightClick(view, 1000, 700);
    const shown = (await view.until("the menu", () => menu(view))) as HTMLElement;
    await view.until("placed inside", () => shown.style.left === "820px" && shown.style.top === "464px");
  });

  test("a right-click on the canvas around the page opens the menu too, with Paste", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.mouse("contextmenu", view.query(".hb-canvas"), 5, 5, { button: 2 });
    await view.until("the menu", () => menu(view));
    assert.ok(item(view, "paste"), "paste");
    assert.ok(!item(view, "delete"), "nothing is selected to delete");
  });

  test("Escape and a press elsewhere close the menu", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    rightClick(view, 60, 60);
    await view.until("the menu", () => menu(view));
    view.key("Escape");
    await view.until("closed by Escape", () => !menu(view));
    rightClick(view, 60, 60);
    await view.until("the menu", () => menu(view));
    view.mouse("mousedown", view.document.body, 5, 5);
    await view.until("closed by a press elsewhere", () => !menu(view));
  });
});
