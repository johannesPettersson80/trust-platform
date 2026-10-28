import * as assert from "assert";

import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import { object, Webview } from "./hmiBuilderWebview";

// Specification 39, §5.3 item 6: in Design mode the inspector's Preview shows the selected object
// in each of its states, including stale and unknown, without a connected source; a state that
// needs a binding the object lacks says which. A preview is never saved.

const PUMP = object("p-101", "pump", [40, 40], [128, 108], {
  tag: "P-101",
  bind: {
    running: { path: "Plant.P101Running", direction: "read" },
    fault: { path: "Plant.P101Fault", direction: "read" },
  },
});
const PAGE: HmiPageModel = { title: "Preview", object: [PUMP], pipe: [] };

function drawnState(view: Webview): string | null {
  return (
    view
      .query('[data-part="canvas-page"] [data-object="p-101"] .hmi-equipment, [data-part="canvas-page"] .hmi-equipment')
      ?.getAttribute("data-equipment") ?? null
  );
}

suite("HMI Builder editor: state preview", function () {
  this.timeout(20000);

  test("Build shows no fabricated running equipment or trend traces", async () => {
    const trend = object("history", "trend", [40, 180], [600, 240], {
      label: "Level history",
      min: 0,
      max: 3,
      span: 300,
      bind: { Level: { path: "Tank.Level", direction: "read" } },
    });
    const view = new Webview();
    await view.loaded({ title: "Static Build", object: [PUMP, trend], pipe: [] });
    assert.strictEqual(view.document.querySelectorAll('[data-part="canvas-page"] .hmi-trend polyline').length, 0);
    assert.strictEqual(view.query('[data-part="canvas-page"] .hmi-spin'), null);
    assert.strictEqual(view.query('[data-part="design-sample-notice"]'), null);
    assert.strictEqual(view.edits().length, 0, "static presentation preserves authored bindings");
    view.query('[data-action="live"]').click();
    await view.until("Live mode's status", () => view.query('[data-part="live-status"]'));
    await new Promise((resolve) => setTimeout(resolve, 150));
    assert.strictEqual(view.query('[data-part="design-sample-notice"]'), null);
    assert.strictEqual(
      view.document.querySelectorAll('[data-part="canvas-page"] .hmi-trend polyline').length,
      0,
      "Live mode, with no run target yet, does not borrow fabricated history",
    );
    assert.strictEqual(view.query('[data-action="live-source"]'), null, "Live mode has one source: the run target");
  });

  test("the inspector's Preview draws the selected pump in each of its states, and never edits the page", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.mouse("mousedown", view.layer(), 100, 80);
    view.mouse("mouseup", view.window, 100, 80);
    const preview = await view.until("the Preview row", () => view.query('[data-part="preview"]'));
    const names = [...preview.querySelectorAll("button")].map((button: { textContent: string }) => button.textContent);
    assert.deepStrictEqual(names, [
      "Build",
      "Running",
      "Stopped",
      "Fault",
      "Manual",
      "Out of service",
      "Stale",
      "Unknown",
    ]);
    const before = view.edits().length;
    view.query('[data-part="preview"] [data-state="fault"]').click();
    await view.until("the pump drawn in fault", () => drawnState(view) === "fault" || undefined);
    view.query('[data-part="preview"] [data-state="stale"]').click();
    await view.until("the stale outline", () => view.query('[data-part="canvas-page"] [data-part="stale-outline"]'));
    view.query('[data-part="preview"] [data-state="unknown"]').click();
    await view.until(
      "the pump unknown",
      () =>
        view.query('[data-part="canvas-page"] .hmi-equipment [data-part="state"]')?.textContent === "?" || undefined,
    );
    assert.strictEqual(view.edits().length, before, "a preview is not an edit");
  });

  test("a state that needs a binding the object lacks is offered disabled and names it", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.mouse("mousedown", view.layer(), 100, 80);
    view.mouse("mouseup", view.window, 100, 80);
    const manual = await view.until("the Manual state", () =>
      view.query('[data-part="preview"] [data-state="manual"]'),
    );
    assert.strictEqual(manual.disabled, true);
    assert.match(manual.getAttribute("title") ?? "", /manual/, "it names the binding it needs");
    assert.strictEqual(view.query('[data-part="preview"] [data-state="fault"]').disabled, false);
  });
});
