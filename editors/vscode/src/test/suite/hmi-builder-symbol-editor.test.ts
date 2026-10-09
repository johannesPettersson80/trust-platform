import * as assert from "assert";
import { Webview } from "./hmiBuilderWebview";

// Specification 39 §14.4.7: the actual shipped webview selects a drawing part and edits its
// typed rule, using the same artwork; preview values never become document edits.
const SYMBOL = {
  name: "mixer",
  title: "Mixer",
  size: [128, 128],
  viewBox: [0, 0, 100, 100],
  properties: { running: "boolean", speed: "number" },
  ports: [],
  parts: [{ part: "rotor", spin: "running" }],
  svg: {
    tag: "g",
    attrs: {},
    children: [
      {
        tag: "circle",
        attrs: { id: "my-mixer-rotor", "data-part": "rotor", cx: "50", cy: "50", r: "20" },
        children: [],
      },
    ],
  },
};

function send(view: Webview, message: unknown): void {
  view.window.dispatchEvent(new view.window.MessageEvent("message", { data: message }));
}

suite("HMI Builder symbol editor", function () {
  this.timeout(15000);
  test("selects a drawn part and offers only bindings compatible with its rule", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      send(view, { type: "symbol", model: SYMBOL, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.ok(view.query('[data-view="symbol-editor"]'), "a symbol definition opens the visual symbol editor");
      view.query('svg [id="my-mixer-rotor"]').dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
      const select = await view.until("the selected part's rule", () => view.query('select[name="drive"]'));
      assert.strictEqual(select.value, "spin");
      assert.deepStrictEqual(
        Array.from(view.query('select[name="property"]').options, (option: any) => option.value).filter(Boolean),
        ["running"],
      );
      select.value = "angle";
      select.dispatchEvent(new view.window.Event("change", { bubbles: true }));
      await view.until("number bindings", () => view.query('select[name="property"] option[value="speed"]'));
      assert.ok(!view.query('select[name="property"] option[value="running"]'));
      const property = view.query('select[name="property"]');
      property.value = "speed";
      property.dispatchEvent(new view.window.Event("change", { bubbles: true }));
      const edit = await view.until("an edited symbol", () =>
        view.posted.find((message) => message.type === "editSymbol"),
      );
      assert.deepStrictEqual((edit.model as any).parts, [{ part: "rotor", angle: "speed" }]);
    } finally {
      view.window.close();
    }
  });

  test("preview values drive the shared symbol drawing without editing the document", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      send(view, { type: "symbol", model: SYMBOL, theme: "light" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      const input = view.query('input[name="preview-running"]');
      assert.ok(input, "the symbol offers a preview value for each binding");
      view.check(input, true);
      await view.until("the rotor turns", () => view.query("#my-mixer-rotor.hmi-spin"));
      assert.strictEqual(view.posted.filter((message) => message.type === "editSymbol").length, 0);
      assert.ok(view.query(".hmi-theme-light"));
    } finally {
      view.window.close();
    }
  });

  test("changing the selected part clears the old highlight without modifying the symbol model", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      const model = JSON.parse(JSON.stringify(SYMBOL));
      model.svg.children.forEach((node: any) => delete node.attrs["data-picked"]);
      model.svg.children.push({
        tag: "rect",
        attrs: { id: "my-mixer-body", x: "0", y: "0", width: "10", height: "10" },
        children: [],
      });
      const original = JSON.stringify(model);
      send(view, { type: "symbol", model, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      view.query("svg #my-mixer-rotor").dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
      await view.until("rotor selected", () => view.query("#my-mixer-rotor[data-picked]"));
      view.query("svg #my-mixer-body").dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
      await view.until("body selected", () => view.query("#my-mixer-body[data-picked]"));
      assert.strictEqual(
        view.document.querySelectorAll("svg [data-picked]").length,
        1,
        "only the current part is highlighted",
      );
      assert.strictEqual(JSON.stringify(model), original, "selection does not mutate the received drawing");
    } finally {
      view.window.close();
    }
  });

  test("invalid text keeps the drawing and prevents visual edits until valid text returns", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      send(view, { type: "symbol", model: SYMBOL, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      view.query("svg #my-mixer-rotor").dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
      await view.until("the inspector", () => view.query('select[name="drive"]'));
      send(view, { type: "invalid", errors: [{ message: "spin needs a boolean property" }] });
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.ok(
        view.query('[role="alert"]')?.textContent.includes("spin needs a boolean property"),
        "the parser's error is visible",
      );
      assert.ok(view.query("svg #my-mixer-rotor"), "the last valid drawing stays visible");
      assert.ok(view.query('select[name="drive"]').disabled, "editing pauses");
      send(view, { type: "symbol", model: SYMBOL, theme: "dark" });
      await view.until("editing resumes", () => !view.query('select[name="drive"]').disabled);
    } finally {
      view.window.close();
    }
  });

  test("bindings and ports can be added in the inspector as symbol document edits", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      send(view, { type: "symbol", model: SYMBOL, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      const add = view.query('[data-action="add-binding"]');
      assert.ok(add, "bindings are editable without opening TOML");
      view.type(view.query('input[name="binding-name"]'), "temperature");
      view.choose(view.query('select[name="binding-kind"]'), "number");
      add.click();
      const bindingEdit = await view.until("the new binding", () =>
        view.posted.find((message) => message.type === "editSymbol"),
      );
      assert.strictEqual((bindingEdit.model as any).properties.temperature, "number");
      send(view, { type: "symbol", model: bindingEdit.model, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 50));
      view.type(view.query('input[name="port-name"]'), "inlet");
      view.type(view.query('input[name="port-x"]'), "0");
      view.type(view.query('input[name="port-y"]'), "50");
      view.choose(view.query('select[name="port-direction"]'), "left");
      view.query('[data-action="add-port"]').click();
      const portEdit = await view.until(
        "the new port",
        () => view.posted.filter((message) => message.type === "editSymbol")[1],
      );
      assert.deepStrictEqual((portEdit.model as any).ports, [{ name: "inlet", at: [0, 50], direction: "left" }]);
    } finally {
      view.window.close();
    }
  });

  test("a part's rule can be removed and its level range edited", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      const model = { ...SYMBOL, parts: [{ part: "rotor", level: "speed", range: [0, 100] }] };
      send(view, { type: "symbol", model, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      view.query("svg #my-mixer-rotor").dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
      await view.until("the inspector", () => view.query('select[name="drive"]'));
      assert.ok(view.query('input[name="range-max"]'), "a level rule offers its range");
      view.type(view.query('input[name="range-max"]'), "200");
      const changed = await view.until("the range edit", () =>
        view.posted.find((message) => message.type === "editSymbol"),
      );
      assert.deepStrictEqual((changed.model as any).parts[0].range, [0, 200]);
      send(view, { type: "symbol", model: changed.model, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 50));
      view.query('[data-action="remove-rule"]').click();
      const removed = await view.until(
        "the removed rule",
        () => view.posted.filter((message) => message.type === "editSymbol")[1],
      );
      assert.deepStrictEqual((removed.model as any).parts, []);
    } finally {
      view.window.close();
    }
  });

  test("existing bindings and ports are editable and removable; Open as text stays available", async () => {
    const view = new Webview("symbol");
    try {
      await view.ready();
      send(view, {
        type: "symbol",
        model: { ...SYMBOL, ports: [{ name: "inlet", at: [0, 50], direction: "left" }] },
        theme: "dark",
      });
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.ok(
        view.query('[data-action="edit-port"][data-name="inlet"]'),
        "existing ports can be selected for editing",
      );
      view.query('[data-action="edit-port"]').click();
      await view.until("the port fields", () => view.query('input[name="port-name"]')?.value === "inlet");
      view.type(view.query('input[name="port-y"]'), "40");
      view.query('[data-action="add-port"]').click();
      const edit = await view.until("the port edit", () =>
        view.posted.find((message) => message.type === "editSymbol"),
      );
      assert.deepStrictEqual((edit.model as any).ports, [{ name: "inlet", at: [0, 40], direction: "left" }]);
      send(view, { type: "symbol", model: edit.model, theme: "dark" });
      await new Promise((resolve) => setTimeout(resolve, 50));
      view.query('[data-action="remove-port"][data-name="inlet"]').click();
      const removed = await view.until(
        "the port removed",
        () => view.posted.filter((message) => message.type === "editSymbol")[1],
      );
      assert.deepStrictEqual((removed.model as any).ports, []);
      assert.ok(view.query('select[data-binding="speed"]'), "a binding's kind is editable");
      view.choose(view.query('select[data-binding="speed"]'), "text");
      const bindingEdit = await view.until(
        "the binding kind",
        () => view.posted.filter((message) => message.type === "editSymbol")[2],
      );
      assert.strictEqual((bindingEdit.model as any).properties.speed, "text");
      view.query('[data-action="remove-binding"][data-name="speed"]').click();
      const bindingRemoved = await view.until(
        "the binding removed",
        () => view.posted.filter((message) => message.type === "editSymbol")[3],
      );
      assert.ok(!Object.keys((bindingRemoved.model as any).properties).includes("speed"));
      view.query('[data-action="open-text"]').click();
      assert.ok(view.posted.some((message) => message.type === "openAsText"));
    } finally {
      view.window.close();
    }
  });
});
