import * as assert from "assert";
import { PAGE, Webview } from "./hmiBuilderWebview";

// Catalogue §9: Explorer owns page navigation. Page metadata remains available for links,
// while the canvas editor offers components and an optional object-selection list.
suite("HMI Builder editor: Explorer owns pages", function () {
  this.timeout(20000);
  test("page metadata does not create a duplicate page browser", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({
      type: "pages",
      current: "overview",
      pages: [
        { name: "overview", title: "Overview", hidden: false },
        { name: "alarms", title: "Alarms", hidden: false },
      ],
    });
    assert.strictEqual(view.query('[data-left-tab="pages"]'), null);
    assert.strictEqual(view.query("[data-page-add]"), null);
    const objects = view.query('[data-left-tab="layers"]');
    assert.strictEqual(objects.textContent, "Objects");
    assert.strictEqual(objects.getAttribute("aria-selected"), "false");
    objects.click();
    await view.until("objects opened on request", () => objects.getAttribute("aria-selected") === "true");
    view.query('[data-left-tab="components"]').click();
    await view.until(
      "components restored",
      () => view.query('[data-left-tab="components"]').getAttribute("aria-selected") === "true",
    );
    assert.strictEqual(view.edits().length, 0, "changing the visible tool does not modify the page");
  });
});
