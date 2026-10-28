import * as assert from "assert";

import { PAGE, Webview } from "./hmiBuilderWebview";

// Specification 39, §3.3 item 8: Ctrl+wheel and pinch zoom; Space-drag and middle-drag pan; keys
// show 100 %; a page opens at the zoom the user last left it at. Zooming at the pointer and the
// fit keys need a laid-out page: the screen pass checks them in VS Code.

const zoomShown = (view: Webview): string => view.query(".hb-zoom-value")?.textContent ?? "";
const area = (view: Webview): any => view.query(".hb-canvas");

function wheel(view: Webview, deltaY: number, extra: object = {}): void {
  area(view).dispatchEvent(
    new view.window.WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY,
      clientX: 200,
      clientY: 200,
      ...extra,
    }),
  );
}

suite("HMI Builder editor: zoom and pan", function () {
  this.timeout(20000);

  test("the first view is at least 75 percent; explicit Fit and a saved zoom may be smaller", async () => {
    const view = new Webview();
    try {
      Object.defineProperty(view.window.HTMLElement.prototype, "clientWidth", {
        configurable: true,
        get() {
          return (this as HTMLElement).classList.contains("hb-canvas") ? 760 : 1440;
        },
      });
      Object.defineProperty(view.window.HTMLElement.prototype, "clientHeight", {
        configurable: true,
        get() {
          return (this as HTMLElement).classList.contains("hb-canvas") ? 650 : 900;
        },
      });
      await view.loaded(PAGE);
      await view.until("first viewport scale", () => parseInt(zoomShown(view), 10) < 100);
      assert.strictEqual(zoomShown(view), "75 %", "first-open text is not reduced to the tiny whole-page fit");
      view.key("!", { shiftKey: true, code: "Digit1" });
      await view.until("explicit full-page fit", () => zoomShown(view) === "54 %");
      view.send({ type: "zoom", zoom: 0.41 });
      await view.until("remembered smaller zoom", () => zoomShown(view) === "41 %");
    } finally {
      view.window.close();
    }
  });

  test("Ctrl+wheel and a pinch zoom; a wheel alone scrolls", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "zoom", zoom: 1 });
    await view.until("100 %", () => zoomShown(view) === "100 %");
    wheel(view, -100);
    assert.strictEqual(zoomShown(view), "100 %", "a wheel alone does not zoom");
    wheel(view, -100, { ctrlKey: true });
    await view.until("zoomed in", () => parseInt(zoomShown(view), 10) > 100);
    wheel(view, 400, { ctrlKey: true });
    await view.until("zoomed out", () => parseInt(zoomShown(view), 10) < 100);
  });

  test("Shift+0 shows the page at 100 %", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "zoom", zoom: 0.5 });
    await view.until("50 %", () => zoomShown(view) === "50 %");
    view.key(")", { shiftKey: true, code: "Digit0" });
    await view.until("100 %", () => zoomShown(view) === "100 %");
  });

  test("Space-drag and middle-drag pan the canvas", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    const canvas = area(view);
    canvas.scrollLeft = 100;
    canvas.scrollTop = 100;
    view.key(" ", { code: "Space" });
    view.mouse("mousedown", view.layer(), 300, 300);
    view.mouse("mousemove", view.window, 260, 270);
    view.mouse("mouseup", view.window, 260, 270);
    view.document.body.dispatchEvent(
      new view.window.KeyboardEvent("keyup", { key: " ", code: "Space", bubbles: true }),
    );
    assert.deepStrictEqual([canvas.scrollLeft, canvas.scrollTop], [140, 130], "the page follows the pointer");
    assert.strictEqual(view.edits().length, 0, "panning edits nothing");
    view.mouse("mousedown", view.layer(), 300, 300, { button: 1 });
    view.mouse("mousemove", view.window, 320, 310, { button: 1 });
    view.mouse("mouseup", view.window, 320, 310, { button: 1 });
    assert.deepStrictEqual([canvas.scrollLeft, canvas.scrollTop], [120, 120]);
  });

  test("a zoom the user chooses is remembered for the page", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "zoom", zoom: 1 });
    await view.until("100 %", () => zoomShown(view) === "100 %");
    (view.query('[title="Zoom in"]') as any).click();
    await view.until("the zoom sent to be remembered", () =>
      view.posted.some((message) => message.type === "zoom" && Number(message.zoom) > 1),
    );
  });
});
