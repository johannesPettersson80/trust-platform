import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import type { ProjectTexts } from "../../hmiBuilder/editor/messages";
import { PAGE, Webview } from "./hmiBuilderWebview";

// Specification 39, §21.3 item 5: the top bar never wraps; the page title truncates with an
// ellipsis and shows its full name on hover; below 1100 px the less frequent actions move into a
// More menu, a menu button that Escape closes.

const STYLES = path.resolve(__dirname, "../../../media/hmiBuilderWebview.css");
const LONG_TITLE = "Secondary clarifier and sludge return pumping station";
const TEXTS: ProjectTexts = {
  languages: [
    { tag: "en", name: "English" },
    { tag: "sv", name: "Svenska" },
  ],
  texts: [{ text: LONG_TITLE, places: ["overview"] }],
  words: [],
  project: {},
  builtin: {},
  errors: [],
};

/** The webview with its shipped styles and a page with a long title, its top bar `width` pixels
 * wide. jsdom lays nothing out: the bar's width is given, as the editor's window gives it. */
async function barAt(width: number): Promise<Webview> {
  const view = new Webview();
  assert.ok(fs.existsSync(STYLES), "the webview styles are built (npm run build:hmi-builder)");
  const styles = view.document.createElement("style");
  styles.textContent = fs.readFileSync(STYLES, "utf8");
  view.document.head.appendChild(styles);
  assert.ok(styles.sheet && styles.sheet.cssRules.length > 0, "the shipped styles are read");
  const laidOut = Object.getOwnPropertyDescriptor(view.window.Element.prototype, "clientWidth");
  Object.defineProperty(view.window.HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get(this: any) {
      return this.classList?.contains("hb-topbar") ? width : (laidOut?.get?.call(this) ?? 0);
    },
  });
  await view.loaded({ ...PAGE, title: LONG_TITLE });
  view.send({ type: "texts", texts: TEXTS });
  view.window.dispatchEvent(new view.window.Event("resize"));
  await view.until("the language picker", () => view.query('[data-part="language-picker"]'));
  return view;
}

const computed = (view: Webview, element: any, property: string): string =>
  view.window.getComputedStyle(element).getPropertyValue(property);

suite("HMI Builder editor: the top bar (§21.3 item 5)", function () {
  this.timeout(20000);

  test("at 900 px the bar keeps one row with the title cut and named in full, and the less frequent actions are in More", async () => {
    const view = await barAt(900);
    const bar = await view.until("the narrow bar", () => view.query('.hb-topbar[data-compact="true"]'));
    assert.strictEqual(computed(view, bar, "flex-wrap"), "nowrap", "the bar never wraps");
    const title = bar.querySelector(".hb-title");
    assert.strictEqual(title.textContent, LONG_TITLE);
    assert.strictEqual(title.getAttribute("title"), LONG_TITLE, "the full name shows on hover");
    assert.deepStrictEqual(
      ["overflow", "text-overflow", "white-space"].map((property) => computed(view, title, property)),
      ["hidden", "ellipsis", "nowrap"],
      "the title is cut with an ellipsis on its one line",
    );
    for (const action of ["create-plc-code", "create-hmi", "languages", "open-as-text", "longest"]) {
      assert.strictEqual(bar.querySelector(`[data-action="${action}"]`), null, `${action} is not in the narrow bar`);
    }
    assert.ok(bar.querySelector('[data-action="deploy"]'), "Deploy stays");
    assert.ok(bar.querySelector('[data-part="language-picker"]'), "the language shown stays");
    assert.ok(bar.querySelector('[title="Zoom in"]'), "zoom stays");

    const more = bar.querySelector('[data-action="more"]');
    assert.ok(more, "a More button");
    assert.strictEqual(more.getAttribute("aria-haspopup"), "menu");
    assert.strictEqual(more.getAttribute("aria-expanded"), "false");
    more.click();
    const menu = await view.until("the More menu", () => view.query('[data-part="more-menu"][role="menu"]'));
    assert.strictEqual(more.getAttribute("aria-expanded"), "true");
    assert.strictEqual(more.getAttribute("aria-controls"), menu.id);
    const items = [...menu.querySelectorAll("[data-action]")];
    assert.deepStrictEqual(
      items.map((item: any) => item.getAttribute("data-action")),
      ["create-plc-code", "create-hmi", "languages", "longest", "open-as-text"],
    );
    assert.deepStrictEqual(
      items.map((item: any) => item.getAttribute("role")),
      ["menuitem", "menuitem", "menuitem", "menuitemcheckbox", "menuitem"],
    );
    assert.strictEqual(menu.querySelector('[data-action="longest"]').getAttribute("aria-checked"), "false");
    await view.until("the first item focused", () => view.document.activeElement === items[0]);

    // Escape closes the menu and gives the focus back to its button.
    items[0].dispatchEvent(
      new view.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
    await view.until("the menu closed", () => !view.query('[data-part="more-menu"]'));
    assert.strictEqual(more.getAttribute("aria-expanded"), "false");
    assert.strictEqual(view.document.activeElement, more, "the focus is back on More");

    // An item does what its button does in a wide bar, and the menu closes.
    more.click();
    (
      await view.until("Create PLC code", () => view.query('[data-part="more-menu"] [data-action="create-plc-code"]'))
    ).click();
    await view.until("the request", () => view.posted.some((message) => message.type === "createPlcCode"));
    await view.until("the menu closed after its action", () => !view.query('[data-part="more-menu"]'));
  });

  test("at 1280 px every action is in the bar and there is no More menu", async () => {
    const view = await barAt(1280);
    const bar = await view.until("the full bar", () => view.query('.hb-topbar[data-compact="false"]'));
    for (const action of ["create-plc-code", "create-hmi", "open-as-text", "longest", "deploy"]) {
      assert.ok(bar.querySelector(`[data-action="${action}"]`), `${action} in the bar`);
    }
    assert.strictEqual(bar.querySelector('[data-action="more"]'), null, "no More menu");
    assert.strictEqual(computed(view, bar, "flex-wrap"), "nowrap", "one row here too");
  });
});
