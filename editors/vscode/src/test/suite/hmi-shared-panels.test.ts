import * as assert from "assert";
import { Webview, PAGE, object } from "./hmiBuilderWebview";
import { withSharedPanels } from "../../hmiBuilder/editor/panelPresentation";
import { renamedReferences } from "../../hmiBuilder/editor/explorerFiles";
import { screenCoordinates } from "../../hmiBuilder/drawing/regionCoordinates";
import { drawPage } from "../../hmiBuilder/drawing";
import { renderToString } from "../../hmiBuilder/drawing/svg";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";

function composition(): HmiPageModel {
  return {
    ...PAGE,
    object: [
      ...PAGE.object,
      object("shared-link", "page-button", [16, 32], [180, 48], { text: "Overview", click: { page: "overview" } }),
    ],
    regions: [
      { panel: null, at: [240, 0], size: [1040, 720], objects: PAGE.object.map((item) => item.id), pipes: [] },
      { panel: "navigation", at: [0, 0], size: [240, 720], objects: ["shared-link"], pipes: [] },
    ],
  };
}

suite("HMI shared panels", function () {
  this.timeout(20000);
  test("drawing clips the content separately and marks the active navigation link", () => {
    const source = composition();
    const svg = renderToString(drawPage(source, { size: [1280, 720], values: {}, currentPage: "overview" }));
    assert.ok(svg.includes('data-shared-panel="navigation"'));
    assert.ok(svg.includes('aria-current="page"'));
    assert.ok(svg.includes('x="240"'));
    assert.deepStrictEqual(source.object[0].at, PAGE.object[0].at);
    const hit = screenCoordinates(source);
    assert.deepStrictEqual(
      hit.object[0].at,
      [PAGE.object[0].at[0] + 240, PAGE.object[0].at[1]],
      "page controls use their actual screen origin",
    );
    assert.deepStrictEqual(hit.object[2].at, [16, 32], "sidebar controls keep their own origin");
    const moved = {
      ...PAGE,
      object: PAGE.object.map((item, index) => (index === 0 ? { ...item, at: [70, 80] as [number, number] } : item)),
    };
    const combined = withSharedPanels(moved, source);
    assert.deepStrictEqual(combined.object[0].at, [70, 80]);
    assert.strictEqual(moved.object.length, 2, "shared objects are never copied into the edited source");
  });
  test("Build selects a sidebar as a whole and offers its shared definition", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "sharedPanels", model: composition() });
    const sidebar = await view.until("shared sidebar selection surface", () =>
      view.query('[data-shared-panel-select="navigation"]'),
    );
    view.mouse("mousedown", sidebar, 30, 60);
    const button = (await view.until("Edit shared panel", () =>
      Array.from(view.document.querySelectorAll("button")).find(
        (node: any) => node.textContent === "Edit shared panel",
      ),
    )) as any;
    button.click();
    assert.ok(view.posted.some((message) => message.type === "openSharedPanel" && message.name === "navigation"));
    assert.strictEqual(view.edits().length, 0, "selecting shared content never writes a duplicate into a page");
  });
  test("panel assignment is separate from its button destinations", async () => {
    const view = new Webview();
    const panel: HmiPageModel = {
      ...PAGE,
      size: [240, 720],
      fields: {},
      panel: { scope: "all", pages: [], dock: "left", order: 0 },
    };
    await view.loaded(panel);
    view.send({ type: "targets", pages: ["overview", "alarms", "settings"], popups: [] });
    const scope = await view.until("panel scope", () => view.query('[name="panel-scope"]'));
    view.choose(scope, "selected");
    const changed = await view.nextEdit(0);
    assert.deepStrictEqual(changed.panel, { scope: "selected", pages: [], dock: "left", order: 0 });
    assert.deepStrictEqual(changed.object, PAGE.object);
  });
  test("Explorer rename updates shared assignments and links without altering unrelated text", () => {
    const source = {
      title: "overview",
      start_page: "overview",
      panel: { scope: "selected", pages: ["overview", "alarms"] },
      object: [{ id: "nav", text: "overview", click: { page: "overview" } }],
    };
    const renamed = renamedReferences(source, "pages", "overview", "process");
    assert.strictEqual(renamed.start_page, "process");
    assert.deepStrictEqual(renamed.panel, { scope: "selected", pages: ["process", "alarms"] });
    assert.deepStrictEqual(renamed.object, [{ id: "nav", text: "overview", click: { page: "process" } }]);
    assert.strictEqual(source.start_page, "overview");
  });
  test("screen settings edits stay separate from page layout", async () => {
    const view = new Webview("screen");
    await view.ready();
    view.send({
      type: "screenDocument",
      model: {
        size: [1280, 720],
        theme: "dark",
        symbols: "flat",
        languages: ["en"],
        navigation: { style: "none", position: "left" },
      },
      pages: [{ name: "alarms", title: "Alarms", hidden: false }],
    });
    const start = await view.until("start page choice", () => view.query('[name="screen-start-page"]'));
    view.choose(start, "alarms");
    assert.ok(
      view.posted.some((message) => message.type === "editScreen" && (message.changes as any)?.start_page === "alarms"),
    );
    assert.strictEqual(view.edits().length, 0);
  });
});
