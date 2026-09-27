import * as assert from "assert";
import * as vscode from "vscode";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";
import { pollValue } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered example focus", function () {
    this.timeout(90_000);

    test("CPEX-005 preserves focus through search, filters, and reset", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      await extension.activate();
      const gallery = await openRenderedExamplesGallery();
      try {
        const inner = gallery.inner;
        await pollValue("browse all examples", () => inner("var b=d.querySelector('button[data-action-type=browseExamples]');if(!b)return 'WAIT';b.click();return 'CLICKED';"), (value) => value === "CLICKED", 10_000);
        await pollValue("search rendered", () => inner("var e=d.querySelector('#search');if(!e)return 'WAIT';var r=e.getBoundingClientRect();return String(r.width>0&&r.height>0&&!e.closest('[hidden]')&&!e.closest('.is-hidden'));"), (value) => value === "true", 10_000);

        await gallery.focusWebview();
        const searchFrame = await gallery.webviewFrameRect();
        const searchRect = JSON.parse(await inner("var r=d.querySelector('#search').getBoundingClientRect();return JSON.stringify({x:r.x,y:r.y,w:r.width,h:r.height});"));
        assert.ok(searchFrame, "the rendered gallery must expose a positioned webview frame");
        await gallery.clickWebviewPoint(searchFrame.x + searchRect.x + searchRect.w / 2, searchFrame.y + searchRect.y + searchRect.h / 2);
        for (const key of ["T", "w", "i", "n", "C", "A", "T"]) await gallery.pressKey(key, `Key${key.toUpperCase()}`);
        assert.strictEqual(await inner("return d.querySelector('#search').value;"), "TwinCAT");
        assert.strictEqual(await inner("return d.activeElement.id;"), "search",
          "typing a search query must retain focus in the search field");

        await inner("var s=d.querySelector('select[data-filter-kind=hardware]');s.focus();return d.activeElement.dataset.filterKind;");
        await gallery.pressKey("ArrowDown", "ArrowDown");
        assert.strictEqual(await inner("return d.activeElement.dataset.filterKind;"), "hardware",
          "changing the hardware filter must retain select focus");
        await gallery.pressKey("Enter", "Enter");
        await inner("var s=d.querySelector('select[data-filter-kind=category]');s.focus();return d.activeElement.dataset.filterKind;");
        await gallery.pressKey("ArrowDown", "ArrowDown");
        assert.strictEqual(await inner("return d.activeElement.dataset.filterKind;"), "category",
          "changing the category filter must retain select focus");

        await inner("var s=d.querySelector('#search');s.focus();s.value='__no_match__';s.dispatchEvent(new s.ownerDocument.defaultView.Event('input',{bubbles:true}));return s.value;");
        await pollValue("empty reset action", () => inner("return String(Boolean([...d.querySelectorAll('button')].find(x=>x.textContent==='Clear search and filters')));"), (value) => value === "true", 10_000);
        const frame = await gallery.webviewFrameRect();
        const resetRect = JSON.parse(await inner("var r=[...d.querySelectorAll('button')].find(x=>x.textContent==='Clear search and filters').getBoundingClientRect();return JSON.stringify({x:r.x,y:r.y,w:r.width,h:r.height});"));
        assert.ok(frame, "the rendered gallery must expose a positioned webview frame");
        await gallery.clickWebviewPoint(frame.x + resetRect.x + resetRect.w / 2, frame.y + resetRect.y + resetRect.h / 2);
        assert.strictEqual(await inner("return [d.querySelector('#search').value,d.querySelector('select[data-filter-kind=hardware]').value,d.querySelector('select[data-filter-kind=category]').value].join('|');"), "|all|all");
        assert.strictEqual(await inner("return d.activeElement.id;"), "search",
          "reset must return focus to the search field");
      } finally {
        gallery.close();
      }
    });
  });
}
