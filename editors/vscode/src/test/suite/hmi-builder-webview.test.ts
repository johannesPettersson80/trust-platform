import * as assert from "assert";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { Arrangement } from "../../hmiBuilder/editor/arrangement";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import { AGITATOR } from "./hmi-drawing-my-symbols.test";
import { CATALOGUE, object, PAGE, SCREEN, Webview } from "./hmiBuilderWebview";
import { penName } from "../../hmiBuilder/editor/objectName";

// Specification 39, §3.2–3.4: the HMI Builder webview, as it is shipped (media/hmiBuilderWebview.js),
// driven with mouse and keyboard events in a DOM.

const find = (page: HmiPageModel, id: string): HmiObjectModel | undefined => page.object.find((item) => item.id === id);

suite("HMI Builder editor: webview", function () {
  test("the canvas outlines the objects with binding problems and clears them when fixed", async () => {
    const view = new Webview();
    await view.loaded();
    const rendered = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150));
    view.send({ type: "problems", problems: { speed: ['"Main.P101.Speed" is not a variable of the program'] } });
    await rendered();
    const flagged = view.query('svg [data-object="speed"]');
    assert.strictEqual(flagged?.getAttribute("data-problems"), "true", "the problem on the canvas");
    assert.strictEqual(
      flagged.querySelector("title")?.textContent,
      '"Main.P101.Speed" is not a variable of the program',
    );
    assert.strictEqual(view.query('svg [data-object="level"]')?.getAttribute("data-problems"), null);
    view.send({ type: "problems", problems: {} });
    await rendered();
    assert.strictEqual(view.query('svg [data-object="speed"]')?.getAttribute("data-problems"), null, "cleared");
  });

  test("an object with a quality warning carries the warning sign, and the inspector lists it with No unit", async () => {
    const view = new Webview();
    await view.loaded();
    const rendered = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150));
    const warning = {
      kind: "unit" as const,
      object: "speed",
      message: "Pump speed shows a REAL value without a unit.",
    };
    // The key handler for the loaded page is attached just after the page is drawn.
    await new Promise((resolve) => setTimeout(resolve, 50));
    view.send({ type: "problems", problems: {}, warnings: { speed: [warning] } });
    await rendered();
    const marked = view.query('svg [data-object="speed"]');
    assert.strictEqual(marked?.getAttribute("data-warnings"), "true", "the warning on the canvas");
    assert.ok(marked.querySelector('[data-part="warning"]'), "its sign");
    assert.strictEqual(view.query('svg [data-object="level"]')?.getAttribute("data-warnings"), null);
    view.key("Tab");
    await rendered();
    const quality = view.query('[data-part="quality"]');
    assert.match(quality?.textContent ?? "", /without a unit/, "the inspector lists it");
    const before = view.edits().length;
    const noUnit = view.query('[data-action="no-unit"]');
    assert.ok(noUnit, "No unit");
    noUnit.click();
    const edited = await view.nextEdit(before);
    assert.strictEqual(find(edited, "speed")?.unit, "", 'unit = "" says the number has none');
  });

  test("with nothing selected the Colours section lists the theme's tokens, writes a chosen colour, resets it, and shows colour warnings", async () => {
    const view = new Webview();
    await view.loaded();
    const rendered = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150));
    view.send({
      type: "screen",
      screen: { size: [1280, 720], theme: "dark", colors: { dark: { accent: "#f97316" } } },
    });
    view.send({
      type: "colourWarnings",
      warnings: [
        {
          kind: "look-alike" as const,
          colour: { theme: "dark" as const, token: "accent" },
          message: "accent looks like the medium-priority alarm colour in the dark theme (0.06 apart).",
        },
      ],
    });
    await rendered();
    const colours = view.query('[data-part="colours"]');
    assert.ok(colours, "the Colours section");
    assert.strictEqual(colours.open, true, "open while a colour has a warning");
    assert.strictEqual(view.query('input[name="colour-accent"]')?.value, "#f97316", "the project's colour");
    assert.strictEqual(
      view.query('input[name="colour-muted"]')?.value,
      "#8a94a8",
      "the default where the project sets none",
    );
    assert.match(view.query('[data-part="colour-warning"][data-token="accent"]')?.textContent ?? "", /looks like/);
    const muted = view.query('input[name="colour-muted"]');
    const setter = Object.getOwnPropertyDescriptor(view.window.HTMLInputElement.prototype, "value")!.set!;
    setter.call(muted, "#99a1b3");
    muted.dispatchEvent(new view.window.Event("change", { bubbles: true }));
    await rendered();
    assert.deepStrictEqual(view.posted.filter((message: any) => message.type === "colour").pop(), {
      type: "colour",
      theme: "dark",
      token: "muted",
      value: "#99a1b3",
    });
    assert.strictEqual(
      view.query('[data-action="reset-colour"][data-token="muted"]'),
      null,
      "only a project colour can be reset",
    );
    view.query('[data-action="reset-colour"][data-token="accent"]').click();
    await rendered();
    assert.deepStrictEqual(view.posted.filter((message: any) => message.type === "colour").pop(), {
      type: "colour",
      theme: "dark",
      token: "accent",
    });
  });

  test("Add pen adds a variable to a trend as a pen named as its legend reads, numbered when taken", async () => {
    const trendPage: HmiPageModel = {
      title: "Overview",
      object: [object("history", "trend", [40, 40], [480, 240])],
      pipe: [],
    };
    const view = new Webview();
    await view.loaded(trendPage);
    await new Promise((resolve) => setTimeout(resolve, 50));
    view.key("Tab");
    await new Promise((resolve) => setTimeout(resolve, 150));
    const before = view.edits().length;
    assert.ok(view.query('input[name="add-pen"]'), "Add pen");
    view.type(view.query('input[name="add-pen"]'), "Tank.Level");
    const first = await view.nextEdit(before);
    assert.deepStrictEqual(find(first, "history")?.bind, { Level: { path: "Tank.Level", direction: "read" } });
    assert.strictEqual(
      view.query('input[name="add-pen"]').value,
      "",
      "the field empties, so leaving it does not add the pen again",
    );
    view.send({ type: "page", model: first });
    await new Promise((resolve) => setTimeout(resolve, 150));
    view.type(view.query('input[name="add-pen"]'), "Other.Level");
    const second = await view.nextEdit(before + 1);
    assert.deepStrictEqual(Object.keys(find(second, "history")?.bind ?? {}), ["Level", "Level 2"]);
    view.send({ type: "page", model: second });
    await new Promise((resolve) => setTimeout(resolve, 150));
    view.type(view.query('input[name="add-pen"]'), "Main.TT101.Value");
    const third = await view.nextEdit(before + 2);
    assert.deepStrictEqual(find(third, "history")?.bind.TT101, { path: "Main.TT101.Value", direction: "read" });
  });

  test("a pen's name: the tag for a value, the tag and member in words, a program's variable in words", () => {
    assert.strictEqual(penName("Main.TT101.Value"), "TT101");
    assert.strictEqual(penName("Main.TIC101.Setpoint"), "TIC101 setpoint");
    assert.strictEqual(penName("Main.EH101.PowerOutput"), "EH101 power output");
    assert.strictEqual(penName("Simulation.WindowOpening"), "Window opening");
    assert.strictEqual(penName("Tank.Level"), "Level");
  });

  this.timeout(20000);

  test("the webview asks for the page, then shows the components by group and the page on the canvas", async () => {
    const view = new Webview();
    await view.ready();
    assert.strictEqual(view.posted.filter((message) => message.type === "ready").length, 1, "one initial page request");
    assert.ok(
      view.posted.some(
        (message) => message.type === "selection" && Array.isArray(message.ids) && message.ids.length === 0,
      ),
      "initial empty selection has explicit provenance",
    );
    await view.loaded();
    for (const type of ["value", "gauge", "bar", "panel"]) assert.ok(view.query(`[data-component="${type}"]`), type);
    const groups = [...view.document.querySelectorAll("[data-component-group]")].map((group: any) =>
      group.getAttribute("data-component-group"),
    );
    // Without component definitions, equipment pictures are symbols (§21.3 item 1).
    assert.deepStrictEqual(groups, ["shapes", "symbols", "display", "control"]);
    assert.ok(view.query('[data-component-group="symbols"] [data-component="pump"]'), "the pump among the symbols");
    assert.ok(
      view.query('[data-component-group="symbols"] [data-component="temperature-sensor"]'),
      "the sensor among the symbols",
    );
    assert.strictEqual(view.query('[data-component-group="symbols"] h2')?.textContent, "Symbols");
    assert.ok(view.query('svg [data-object="level"]'));
    const page = view.query('[data-part="canvas-page"]');
    assert.strictEqual(page.style.width, "1280px");
    assert.strictEqual(page.style.height, "720px");
  });

  test("equipment with a component definition comes first, listed by what it is (§21.3 item 1)", async () => {
    const view = new Webview();
    await view.ready();
    const definition = (block: string, symbol: string, position: "measurements" | "actuators") => ({
      block,
      title: block.split(".").pop()!,
      symbol,
      panel: "plant-run",
      variants: [],
      bindings: {},
      fields: {},
      settings: [],
      io: [],
      dependencies: [],
      position,
    });
    const catalogue = {
      ...CATALOGUE,
      equipment: [
        definition("Plant.FB_Pump", "pump", "actuators"),
        definition("Plant.FB_TemperatureInput", "temperature-sensor", "measurements"),
      ],
    } as Catalogue;
    view.send({ type: "catalogue", catalogue });
    view.send({ type: "screen", screen: SCREEN });
    view.send({ type: "page", model: PAGE });
    await view.until("the page on the canvas", () => view.query('svg [data-object="speed"]'));
    const groups = [...view.document.querySelectorAll("[data-component-group]")].map((group: any) =>
      group.getAttribute("data-component-group"),
    );
    assert.deepStrictEqual(groups, ["equipment", "instruments", "shapes", "symbols", "display", "control"]);
    assert.ok(
      view.query('[data-component-group="equipment"] [data-block="Plant.FB_Pump"]'),
      "the pump under Equipment",
    );
    assert.ok(
      view.query('[data-component-group="instruments"] [data-block="Plant.FB_TemperatureInput"]'),
      "the transmitter under Instruments",
    );
    assert.strictEqual(view.query('[data-component-group="equipment"] h2')?.textContent, "Equipment");
  });

  test("the project's own symbols are offered under My Symbols by their titles, and drawn on the canvas", async () => {
    const view = new Webview();
    await view.ready();
    const tank3 = { ...AGITATOR, name: "tank3", title: "Tank 3" };
    const component = {
      type: "my:tank3",
      category: "equipment",
      group: "my-symbols",
      phase: 3,
      size: [128, 108],
      ports: ["inlet"],
      properties: { running: { accepts: ["boolean"], directions: ["read"] } },
    };
    view.send({
      type: "catalogue",
      catalogue: { ...CATALOGUE, components: [...CATALOGUE.components, component] } as Catalogue,
      symbols: [tank3],
    });
    view.send({ type: "screen", screen: SCREEN });
    view.send({
      type: "page",
      model: { ...PAGE, object: [...PAGE.object, object("t3", "my:tank3", [500, 40], [128, 108], { tag: "T-3" })] },
    });
    await view.until("the page on the canvas", () => view.query('svg [data-object="t3"]'));
    assert.strictEqual(view.query('[data-component-group="my-symbols"] h2')?.textContent, "My Symbols");
    assert.strictEqual(view.query('[data-component="my:tank3"] .hb-card-name')?.textContent, "Tank 3");
    assert.ok(view.query('svg [data-object="t3"] [data-part="rotor"]'), "drawn from its drawing");
  });

  test("a P&ID shows its symbols in the page's theme without states, offers what a P&ID holds, and has no Live mode", async () => {
    const view = new Webview();
    await view.ready();
    view.send({ type: "catalogue", catalogue: CATALOGUE });
    view.send({ type: "screen", screen: SCREEN });
    view.send({ type: "view", view: "pid" });
    view.send({
      type: "page",
      model: {
        title: "Tank farm",
        object: [
          object("p1", "pump", [40, 40], [128, 108], {
            tag: "P101",
            bind: { running: { path: "Main.Running", direction: "read" } },
          }),
        ],
        pipe: [],
      },
    });
    await view.until("the pump on the canvas", () => view.query('svg [data-object="p1"]'));
    assert.ok(view.query(".hb-drawing.hmi-theme-dark"), "the screen's theme, as its pages have");
    assert.strictEqual(view.query(".hmi-theme-pid"), null, "no style of its own");
    assert.strictEqual(
      view.query('svg [data-object="p1"] [data-part="state"]')?.textContent ?? "",
      "",
      "no sample state",
    );
    assert.strictEqual(view.query('svg [data-object="p1"] [data-part="tag"]')?.textContent, "P101", "its tag");
    assert.strictEqual(view.query('[data-action="live"]'), null, "no Live mode");
    assert.strictEqual(view.query('[data-action="preview-size"]'), null, "no screen sizes");
    assert.strictEqual(view.query('[data-component="value"]'), null, "no displays");
    assert.ok(view.query('[data-component="pump"]'), "equipment");
  });

  test("a variable-speed drive has its readable name in Components", async () => {
    const view = new Webview();
    await view.ready();
    const vfd = { type: "vfd", category: "equipment", group: "electrical", phase: 3, size: [128, 108], properties: {} };
    view.send({
      type: "catalogue",
      catalogue: { ...CATALOGUE, components: [...CATALOGUE.components, vfd] } as Catalogue,
    });
    view.send({ type: "screen", screen: SCREEN });
    view.send({ type: "page", model: PAGE });
    const card = await view.until("the VFD's card", () => view.query('[data-component="vfd"]'));
    assert.strictEqual(card.querySelector(".hb-card-name")?.textContent, "Variable-speed drive");
  });

  test("a text component's picture shows its own name", async () => {
    const view = new Webview();
    await view.loaded();
    const picture = view.query('[data-component="text"] .hb-card-preview');
    assert.ok(picture.textContent.includes("Text"), picture.textContent);
  });

  test("Build shows a static placeholder rather than a running value", async () => {
    const view = new Webview();
    await view.loaded();
    assert.ok(
      view.query('[data-object="speed"]').textContent.includes("—"),
      view.query('[data-object="speed"]').textContent,
    );
    assert.ok(!view.query('[data-object="speed"]').textContent.includes("60"));
  });

  test("clicking a component adds it at the centre of the page and selects it", async () => {
    const view = new Webview();
    await view.loaded();
    view.mouse("click", view.query('[data-component="gauge"]'), 0, 0);
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(find(edit, "gauge-1"), object("gauge-1", "gauge", [560, 280], [160, 160]));
    await view.until("the new object selected", () => view.query('[data-selection="gauge-1"]'));
  });

  test("clicking an object selects it, and dragging moves it on the grid", async () => {
    const view = new Webview();
    await view.loaded();
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("speed selected", () => view.query('[data-selection="speed"]'));
    assert.strictEqual(view.edits().length, 0, "a click without moving edits nothing");
    view.drag(view.layer(), [50, 50], [63, 56]);
    const edit = await view.nextEdit(0);
    assert.deepStrictEqual(find(edit, "speed")?.at, [56, 48]);
  });

  test("the arrow keys nudge the selection, and Delete removes it", async () => {
    const view = new Webview();
    await view.loaded();
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("speed selected", () => view.query('[data-selection="speed"]'));
    view.key("ArrowRight");
    assert.deepStrictEqual(find(await view.nextEdit(0), "speed")?.at, [48, 40]);
    view.key("Delete");
    assert.strictEqual(find(await view.nextEdit(1), "speed"), undefined);
  });

  test("a click selects an object that is off the grid without moving it", async () => {
    const view = new Webview();
    await view.loaded({ ...PAGE, object: [...PAGE.object, object("start", "button", [300, 400], [140, 44])] });
    view.drag(view.layer(), [310, 410], [310, 410]);
    await view.until("the button in the inspector", () => view.query('input[name="id"]')?.value === "start");
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.deepStrictEqual(view.edits(), [], "a click is no move");
  });

  test("a resize handle resizes the selected object on the grid", async () => {
    const view = new Webview();
    await view.loaded();
    view.drag(view.layer(), [350, 100], [350, 100]);
    const handle = await view.until("the resize handles", () => view.query('[data-handle="se"]'));
    view.drag(handle, [400, 280], [413, 283]);
    assert.deepStrictEqual(find(await view.nextEdit(0), "level")?.size, [96, 240]);
  });

  test("the inspector edits the selected object's label and position", async () => {
    const view = new Webview();
    await view.loaded();
    view.drag(view.layer(), [50, 50], [50, 50]);
    const label = await view.until("the label field", () => view.query('input[name="label"]'));
    assert.strictEqual(label.value, "Pump speed");
    view.type(label, "Feed pump");
    assert.strictEqual(find(await view.nextEdit(0), "speed")?.label, "Feed pump");
    view.type(view.query('input[name="x"]'), "100");
    assert.deepStrictEqual(find(await view.nextEdit(1), "speed")?.at, [100, 40]);
  });

  test("the inspector binds a property to a variable", async () => {
    const view = new Webview();
    await view.loaded();
    view.drag(view.layer(), [50, 50], [50, 50]);
    const binding = await view.until("the binding field", () => view.query('input[name="bind.value"]'));
    assert.strictEqual(binding.value, "Main.P101.Speed");
    assert.strictEqual(
      binding.closest("label").querySelector(".hb-field-label").textContent,
      "Value",
      "a readable name",
    );
    view.type(binding, "P1.Controller.StateValue.ProcessValue");
    assert.deepStrictEqual(find(await view.nextEdit(0), "speed")?.bind.value, {
      path: "P1.Controller.StateValue.ProcessValue",
      direction: "read",
    });
  });

  test("the inspector edits a control's permission, question, timeout and a selector's options", async () => {
    const view = new Webview();
    await view.loaded({
      ...PAGE,
      object: [
        ...PAGE.object,
        object("fan", "selector", [40, 400], [200, 44], {
          options: ["Off", "Low"],
          bind: { value: { path: "Main.Speed", direction: "read-write" } },
        }),
        object("start", "button", [300, 400], [140, 44], {
          bind: { target: { path: "Main.Start", direction: "write" } },
        }),
      ],
    });
    view.drag(view.layer(), [60, 410], [60, 410]);
    await view.until("the selector in the inspector", () => view.query('input[name="id"]')?.value === "fan");
    const options = view.query('input[name="options"]');
    assert.ok(options, "an options field");
    assert.strictEqual(options.value, "Off, Low");
    view.type(options, "Off, Low , High,");
    assert.deepStrictEqual(find(await view.nextEdit(0), "fan")?.options, ["Off", "Low", "High"]);
    view.choose(view.query('select[name="role"]'), "engineer");
    assert.strictEqual(find(await view.nextEdit(1), "fan")?.role, "engineer");
    view.type(view.query('input[name="confirm"]'), "Change the fan speed?");
    assert.strictEqual(find(await view.nextEdit(2), "fan")?.confirm, "Change the fan speed?");
    view.type(view.query('input[name="timeout"]'), "8");
    assert.strictEqual(find(await view.nextEdit(3), "fan")?.timeout, 8);
    assert.strictEqual(view.query('select[name="action"]'), null, "a selector has no button action");
    view.drag(view.layer(), [310, 410], [310, 410]);
    await view.until("the button in the inspector", () => view.query('input[name="id"]')?.value === "start");
    const action = view.query('select[name="action"]');
    assert.ok(action, "the button's action");
    view.choose(action, "toggle");
    const toggled = await view.nextEdit(4);
    assert.strictEqual(
      find(toggled, "start")?.action,
      "toggle",
      JSON.stringify(
        view
          .edits()
          .slice(4)
          .map((edit) => edit.object.slice(2)),
      ),
    );
    assert.strictEqual(view.query('input[name="options"]'), null, "a button has no options");
  });

  test("with nothing selected, the inspector edits the page's place in the navigation", async () => {
    const view = new Webview();
    await view.loaded();
    await view.until("the page inspector", () => view.query('input[name="title"]'));
    const order = view.query('input[name="order"]');
    assert.ok(order, "an order field");
    view.type(order, "3");
    assert.strictEqual((await view.nextEdit(0)).order, 3);
    view.choose(view.query('select[name="icon"]'), "trend");
    assert.strictEqual((await view.nextEdit(1)).icon, "trend");
    view.type(view.query('input[name="group"]'), "Process");
    assert.strictEqual((await view.nextEdit(2)).group, "Process");
    view.check(view.query('input[name="hidden"]'), true);
    assert.strictEqual((await view.nextEdit(3)).hidden, true);
    assert.strictEqual(view.query('input[name="width"]'), null, "a page takes the screen's size");
  });

  test("a popup is drawn at its own size and its inspector edits the size and the pin", async () => {
    const view = new Webview();
    await view.loaded({ ...PAGE, size: [480, 320], pinnable: false });
    const sheet = await view.until("the canvas", () => view.query('[data-part="canvas-page"] svg.hmi-page'));
    assert.strictEqual(sheet.getAttribute("viewBox"), "0 0 480 320", "the popup's size");
    view.type(view.query('input[name="width"]'), "560");
    assert.deepStrictEqual((await view.nextEdit(0)).size, [560, 320]);
    view.check(view.query('input[name="pinnable"]'), true);
    assert.strictEqual((await view.nextEdit(1)).pinnable, true);
    assert.strictEqual(view.query('input[name="order"]'), null, "a popup has no place in the navigation");
  });

  test("the inspector sends a click to a page or a popup of the project", async () => {
    const view = new Webview();
    await view.loaded();
    view.send({ type: "targets", pages: ["overview", "trends"], popups: ["pump"] });
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("the object in the inspector", () => view.query('input[name="id"]')?.value === "speed");
    const kind = view.query('select[name="click-kind"]');
    assert.ok(kind, "an On click field");
    view.choose(kind, "popup");
    const target = view.query('select[name="click-target"]');
    assert.ok(target, "the popup to open");
    assert.deepStrictEqual(
      [...target.options].map((option: any) => option.value),
      ["pump"],
    );
    assert.deepStrictEqual(find(await view.nextEdit(0), "speed")?.click, { popup: "pump" });
    view.choose(view.query('select[name="click-kind"]'), "page");
    view.choose(view.query('select[name="click-target"]'), "trends");
    assert.deepStrictEqual(find(await view.nextEdit(2), "speed")?.click, { page: "trends" });
    view.choose(view.query('select[name="click-kind"]'), "");
    assert.strictEqual(find(await view.nextEdit(3), "speed")?.click, undefined);
  });

  /** The editor with its page loaded and its key handler attached. */
  async function keyboardReady(): Promise<{ view: Webview; selected: () => string }> {
    const view = new Webview();
    await view.loaded();
    // The key handler for the loaded page is attached just after the page is drawn.
    await new Promise((resolve) => setTimeout(resolve, 50));
    return { view, selected: () => view.query('input[name="id"]')?.value ?? "" };
  }

  const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 100));

  test("Tab and Shift+Tab select the next and the previous object", async () => {
    const { view, selected } = await keyboardReady();
    view.key("Tab");
    await settle();
    assert.strictEqual(selected(), "speed", "Tab selects the first object");
    view.key("Tab");
    await settle();
    assert.strictEqual(selected(), "level");
    view.key("Tab", { shiftKey: true });
    await settle();
    assert.strictEqual(selected(), "speed", "Shift+Tab goes back");
  });

  test("Shift+arrow resizes the selection by a grid step, with Alt by one pixel", async () => {
    const { view } = await keyboardReady();
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("the object selected", () => view.query('input[name="id"]')?.value === "speed");
    view.key("ArrowRight", { shiftKey: true });
    await settle();
    assert.deepStrictEqual(find(view.edits()[0] ?? PAGE, "speed")?.size, [248, 96]);
    view.key("ArrowUp", { shiftKey: true, altKey: true });
    await settle();
    assert.deepStrictEqual(find(view.edits()[1] ?? PAGE, "speed")?.size, [248, 95]);
  });

  test("Enter goes to the inspector", async () => {
    const { view } = await keyboardReady();
    view.drag(view.layer(), [50, 50], [50, 50]);
    await view.until("the object selected", () => view.query('input[name="id"]')?.value === "speed");
    view.key("Enter");
    await settle();
    assert.ok(view.document.activeElement?.closest?.(".hb-inspector"), "focus is in the inspector");
  });

  test("/ goes to the component search, where Enter adds the first match", async () => {
    const { view } = await keyboardReady();
    view.key("/");
    await settle();
    const search = view.document.activeElement;
    assert.strictEqual(search?.getAttribute("name"), "component-search");
    view.type(search, "gauge");
    await settle();
    assert.strictEqual(
      (view.edits()[0] ?? PAGE).object.filter((object) => object.type === "gauge").length,
      1,
      "Enter adds the first match",
    );
  });

  test("text that does not load shows the errors and pauses editing", async () => {
    const view = new Webview();
    await view.loaded();
    view.send({
      type: "invalid",
      errors: [{ file: "hmi/pages/overview.toml", line: 3, message: "unknown field `colour`" }],
    });
    const banner = await view.until("the error banner", () => view.query('[data-part="errors"]'));
    assert.ok(banner.textContent.includes("hmi/pages/overview.toml:3"), banner.textContent);
    assert.ok(banner.textContent.includes("unknown field `colour`"), banner.textContent);
    view.mouse("click", view.query('[data-component="gauge"]'), 0, 0);
    view.key("Delete");
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.strictEqual(view.edits().length, 0, "editing is paused");
    assert.ok(view.query('svg [data-object="speed"]'), "the last page stays on the canvas");
  });

  test("Open as text asks the editor to reopen the file as text", async () => {
    const view = new Webview();
    await view.loaded();
    view.mouse("click", view.query('[data-action="open-as-text"]'), 0, 0);
    await view.until("the request", () => view.posted.some((message) => message.type === "openAsText"));
  });
});

const PICKER_VARIABLES: VariableNode[] = [
  {
    name: "Main",
    path: "Main",
    kind: "program",
    type: "Main",
    kinds: [],
    writable: true,
    children: [
      {
        name: "P101",
        path: "Main.P101",
        kind: "function-block",
        type: "Pump",
        kinds: [],
        writable: true,
        children: [
          {
            name: "Running",
            path: "Main.P101.Running",
            kind: "value",
            type: "BOOL",
            kinds: ["boolean", "value"],
            writable: false,
            writeBlock: "is an output of Pump",
            access: "output",
          },
          {
            name: "Speed",
            path: "Main.P101.Speed",
            kind: "value",
            type: "REAL",
            kinds: ["number", "value"],
            writable: false,
            writeBlock: "is an output of Pump",
            access: "output",
          },
        ],
      },
      { name: "Run", path: "Main.Run", kind: "value", type: "BOOL", kinds: ["boolean", "value"], writable: true },
      { name: "Level", path: "Main.Level", kind: "value", type: "REAL", kinds: ["number", "value"], writable: true },
    ],
  },
];

const PICKER_PAGE: HmiPageModel = {
  ...PAGE,
  object: [...PAGE.object, object("pump-switch", "switch", [40, 400], [96, 48])],
};

/** Lets the webview render what it was sent. */
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 50));

async function withVariables(variables: VariableNode[] = PICKER_VARIABLES, problem?: string): Promise<Webview> {
  const view = new Webview();
  await view.loaded(PICKER_PAGE);
  view.send({ type: "variables", variables, ...(problem ? { problem } : {}) });
  await settle();
  // jsdom lays nothing out: the page sits at the top left at zoom 1.
  const sheet = view.query('[data-part="canvas-page"]');
  sheet.getBoundingClientRect = () => ({
    left: 0,
    top: 0,
    right: 1280,
    bottom: 720,
    width: 1280,
    height: 720,
    x: 0,
    y: 0,
  });
  return view;
}

/** The paths the webview last asked the host to read for its pickers. */
function watched(view: Webview): string[] | undefined {
  const asked = view.posted.filter((message) => message.type === "watch");
  return asked[asked.length - 1]?.paths as string[] | undefined;
}

/** The current value a picker row shows. */
function currentValue(view: Webview, path: string): string | undefined {
  return view.query(`[data-variable="${path}"] [data-part="current-value"]`)?.textContent ?? undefined;
}

function openTags(view: Webview): void {
  const tab = view.query('[data-action="toggle-tags"]');
  assert.ok(tab, "the bottom panel has a Tags tab");
  view.mouse("click", tab, 0, 0);
}

/** A press on a row, a move, and a release over `target` at a screen point. */
function dragRow(view: Webview, path: string, target: any, to: [number, number]): void {
  const row = view.query(`[data-variable="${path}"]`);
  assert.ok(row, `the row ${path}`);
  view.mouse("mousedown", row, 2, 2);
  view.mouse("mousemove", view.window, to[0], to[1]);
  view.mouse("mouseup", target, to[0], to[1]);
}

async function selectSwitch(view: Webview): Promise<void> {
  view.drag(view.layer(), [60, 420], [60, 420]);
  await view.until("the switch selected", () => view.query('[data-selection="pump-switch"]'));
  await settle();
}

suite("HMI Builder editor: variable picker", function () {
  this.timeout(20000);

  test("the Tags panel is closed at first and opens to the program's variables with their types", async () => {
    const view = await withVariables();
    assert.strictEqual(view.query("[data-variable]"), null, "closed at first");
    openTags(view);
    await settle();
    const level = view.query('[data-variable="Main.Level"]');
    assert.ok(level, "the program's variables are listed");
    assert.ok(level.textContent.includes("REAL"), level.textContent);
    assert.strictEqual(view.query('[data-variable="Main.P101.Running"]'), null, "blocks start folded");
    view.mouse("click", view.query('[data-expand="Main.P101"]'), 0, 0);
    await settle();
    assert.ok(view.query('[data-variable="Main.P101.Running"]'), "unfolded");
  });

  test("searching the Tags panel keeps the matching variables", async () => {
    const view = await withVariables();
    openTags(view);
    await settle();
    const search = view.query('input[name="tags-search"]');
    assert.ok(search, "a search field");
    view.type(search, "speed");
    await settle();
    assert.ok(view.query('[data-variable="Main.P101.Speed"]'), "the match, unfolded");
    assert.strictEqual(view.query('[data-variable="Main.Level"]'), null);
  });

  test("when declaration discovery is unavailable, the Tags panel says why", async () => {
    const view = await withVariables([], "main.st:3: expected a type");
    openTags(view);
    await settle();
    const problem = view.query('[data-part="tags-problem"]');
    assert.ok(problem, "the reason is shown");
    assert.ok(problem.textContent.includes("main.st:3: expected a type"), problem.textContent);
  });

  test("dragging a variable onto the page creates the matching component, bound and labelled", async () => {
    const view = await withVariables();
    openTags(view);
    await settle();
    dragRow(view, "Main.Run", view.window, [400, 300]);
    await settle();
    const edit = view.edits()[0];
    assert.ok(edit, "an edit");
    assert.deepStrictEqual(
      find(edit, "status-1"),
      object("status-1", "status", [320, 280], [160, 48], {
        label: "Run",
        bind: { state: { path: "Main.Run", direction: "read" } },
      }),
    );
  });

  test("dropping a function block instance on the page creates nothing and says why", async () => {
    const view = await withVariables();
    openTags(view);
    await settle();
    dragRow(view, "Main.P101", view.window, [400, 300]);
    await settle();
    assert.strictEqual(view.edits().length, 0);
    const notice = view.query('[data-part="notice"]');
    assert.ok(notice, "a notice");
    assert.ok(notice.textContent.includes('"Main.P101" is a function block instance'), notice.textContent);
  });

  test("while a run target is connected, the variables the Tags panel shows have their current values", async () => {
    const view = await withVariables();
    openTags(view);
    await settle();
    assert.deepStrictEqual(watched(view), ["Main.Run", "Main.Level"], "the variables shown, not those folded away");
    view.send({
      type: "pickerValues",
      update: { cycle: 7, values: { "Main.Run": { value: true }, "Main.Level": { value: 4.25 } }, missing: [] },
    });
    await settle();
    assert.strictEqual(currentValue(view, "Main.Run"), "TRUE");
    assert.strictEqual(currentValue(view, "Main.Level"), "4.25");
    view.mouse("click", view.query('[data-expand="Main.P101"]'), 0, 0);
    await settle();
    assert.deepStrictEqual(
      watched(view),
      ["Main.P101.Running", "Main.P101.Speed", "Main.Run", "Main.Level"],
      "unfolded variables are read too",
    );
    view.send({
      type: "pickerValues",
      update: { cycle: 8, values: { "Main.Run": { value: false } }, missing: ["Main.Level"] },
    });
    await settle();
    assert.strictEqual(currentValue(view, "Main.Run"), "FALSE");
    assert.strictEqual(currentValue(view, "Main.Level"), "?", "not in the running program");
    view.send({ type: "pickerValues" });
    await settle();
    assert.ok(!view.query('[data-part="current-value"]'), "no run target, no values");
    openTags(view);
    await settle();
    assert.deepStrictEqual(watched(view), [], "a closed panel reads nothing");
  });

  test("a text variable's current value is quoted, so an empty text shows; a time is not", async () => {
    const texts: VariableNode[] = [
      {
        name: "Main",
        path: "Main",
        kind: "program",
        type: "Main",
        kinds: [],
        writable: true,
        children: [
          {
            name: "Reason",
            path: "Main.Reason",
            kind: "value",
            type: "STRING",
            kinds: ["text", "value"],
            writable: true,
          },
          {
            name: "Label",
            path: "Main.Label",
            kind: "value",
            type: "STRING[20]",
            kinds: ["text", "value"],
            writable: true,
          },
          { name: "Delay", path: "Main.Delay", kind: "value", type: "TIME", kinds: ["text", "value"], writable: true },
        ],
      },
    ];
    const view = await withVariables(texts);
    openTags(view);
    await settle();
    view.send({
      type: "pickerValues",
      update: {
        cycle: 2,
        values: { "Main.Reason": { value: "" }, "Main.Label": { value: "Pump A" }, "Main.Delay": { value: "T#5s" } },
        missing: [],
      },
    });
    await settle();
    assert.strictEqual(currentValue(view, "Main.Reason"), "''");
    assert.strictEqual(currentValue(view, "Main.Label"), "'Pump A'");
    assert.strictEqual(currentValue(view, "Main.Delay"), "T#5s");
  });

  test("a binding's picker shows the current values of the variables it offers", async () => {
    const view = await withVariables();
    await selectSwitch(view);
    view.mouse("click", view.query('[data-action="pick"][data-binding="value"]'), 0, 0);
    await settle();
    assert.deepStrictEqual(watched(view), ["Main.Run"]);
    view.send({ type: "pickerValues", update: { cycle: 7, values: { "Main.Run": { value: false } }, missing: [] } });
    await settle();
    assert.strictEqual(currentValue(view, "Main.Run"), "FALSE");
  });

  test("a group's visibility binding is set on every member from the inspector, offering only booleans", async () => {
    // §18 phase 5 item 2: an object or group shown while a BOOL is TRUE.
    const view = await withVariables();
    view.key("a", { ctrlKey: true });
    await settle();
    const pick = view.query('[data-part="visibility"] [data-action="pick"][data-binding="visible"]');
    assert.ok(pick, "a Visible while binding for the selection");
    assert.ok(view.query('[data-part="visibility"]')?.textContent?.includes("Visible while"));
    view.mouse("click", pick, 0, 0);
    await settle();
    const picker = view.query('[data-part="visibility"] [data-part="picker"]');
    assert.ok(picker?.querySelector('[data-variable="Main.Run"]'), "a boolean is offered");
    assert.ok(!picker?.querySelector('[data-variable="Main.Level"]'), "a number is not");
    view.mouse("click", picker.querySelector('[data-variable="Main.Run"]'), 0, 0);
    await settle();
    const edit = view.edits()[0];
    assert.ok(edit, "an edit");
    assert.deepStrictEqual(
      edit.object.map((candidate: HmiObjectModel) => candidate.bind.visible),
      edit.object.map(() => ({ path: "Main.Run", direction: "read" })),
    );
  });

  test("a single object's visibility binding reads Visible while", async () => {
    const view = await withVariables();
    await selectSwitch(view);
    const row = view.query('[data-binding="visible"]');
    assert.ok(row?.textContent?.includes("Visible while"), row?.textContent ?? "no row");
  });

  test("a binding's picker offers only the variables it can have, and picking one binds it", async () => {
    const view = await withVariables();
    await selectSwitch(view);
    const pick = view.query('[data-action="pick"][data-binding="value"]');
    assert.ok(pick, "a pick button beside the binding");
    view.mouse("click", pick, 0, 0);
    await settle();
    const picker = view.query('[data-part="picker"]');
    assert.ok(picker, "the picker opens");
    assert.ok(picker.querySelector('[data-variable="Main.Run"]'), "a writable boolean is offered");
    assert.strictEqual(picker.querySelector('[data-variable="Main.Level"]'), null, "a number is not");
    assert.strictEqual(
      picker.querySelector('[data-variable="Main.P101.Running"]'),
      null,
      "an output cannot be written",
    );
    view.mouse("click", picker.querySelector('[data-variable="Main.Run"]'), 0, 0);
    await settle();
    assert.deepStrictEqual(find(view.edits()[0], "pump-switch")?.bind.value, {
      path: "Main.Run",
      direction: "read-write",
    });
  });

  test("a variable dropped on a binding it cannot have leaves it unchanged and says why", async () => {
    const view = await withVariables();
    await selectSwitch(view);
    openTags(view);
    await settle();
    const field = view.query('.hb-binding[data-binding="value"] input');
    assert.ok(field, "the binding field");
    dragRow(view, "Main.Level", field, [1100, 200]);
    await settle();
    assert.strictEqual(view.edits().length, 0);
    const notice = view.query('[data-part="notice"]');
    assert.ok(notice, "a notice");
    assert.strictEqual(notice.textContent, `"Main.Level" is REAL; a switch's value needs a boolean`);
  });

  test("a variable dropped on a binding binds it", async () => {
    const view = await withVariables();
    await selectSwitch(view);
    openTags(view);
    await settle();
    dragRow(view, "Main.Run", view.query('.hb-binding[data-binding="value"] input'), [1100, 200]);
    await settle();
    assert.deepStrictEqual(find(view.edits()[0], "pump-switch")?.bind.value, {
      path: "Main.Run",
      direction: "read-write",
    });
  });
});

/** The panel arrangements the webview asked the editor to remember, oldest first. */
function arrangements(view: Webview): Arrangement[] {
  return view.posted
    .filter((message) => message.type === "arrangement")
    .map((message) => (message as unknown as { arrangement: Arrangement }).arrangement);
}

const lastOf = <T>(items: T[]): T | undefined => items[items.length - 1];

/** Sizes the webview's window, as VS Code does when the editor is resized. */
function sized(view: Webview, width: number, height = 1000): void {
  Object.defineProperty(view.window, "innerWidth", { value: width, configurable: true });
  Object.defineProperty(view.window, "innerHeight", { value: height, configurable: true });
  view.window.dispatchEvent(new view.window.Event("resize"));
}

suite("HMI Builder editor: panel arrangement", function () {
  this.timeout(20000);

  test("a side panel collapses to a strip and opens again from it, and the change is remembered", async () => {
    const view = new Webview();
    await view.loaded();
    sized(view, 1600);
    await settle();
    // The side panel hides from any of its tabs, and its tabs name it, so no heading repeats them.
    view.mouse("click", view.query('[data-left-tab="layers"]'), 0, 0);
    await settle();
    assert.ok(view.query('[data-action="collapse-components"]'), "a collapse button on the Layers tab");
    view.mouse("click", view.query('[data-left-tab="components"]'), 0, 0);
    await settle();
    assert.ok(!view.query(".hb-side-left .hb-panel-title"), "no heading under the tabs");
    const collapse = view.query('[data-action="collapse-components"]');
    assert.ok(collapse, "a collapse button on the Components panel");
    view.mouse("click", collapse, 0, 0);
    await settle();
    assert.ok(view.query('[data-part="components-strip"]'), "a strip");
    assert.strictEqual(view.query("[data-component]"), null, "the cards are gone");
    assert.strictEqual(lastOf(arrangements(view))?.left.collapsed, true);
    view.mouse("click", view.query('[data-action="open-components"]'), 0, 0);
    await settle();
    assert.ok(view.query('[data-component="gauge"]'));
    assert.strictEqual(lastOf(arrangements(view))?.left.collapsed, false);
  });

  test("dragging a side panel's edge resizes it within its limits", async () => {
    const view = new Webview();
    await view.loaded();
    sized(view, 1600);
    await settle();
    const edge = view.query('[data-resize="left"]');
    assert.ok(edge, "a resize edge");
    view.drag(edge, [264, 300], [344, 300]);
    await settle();
    assert.strictEqual(lastOf(arrangements(view))?.left.width, 344);
    const columns = view.query(".hb-body").style.gridTemplateColumns;
    assert.ok(columns.startsWith("344px"), columns);
    view.drag(view.query('[data-resize="left"]'), [344, 300], [900, 300]);
    await settle();
    assert.strictEqual(lastOf(arrangements(view))?.left.width, 480, "no wider than 480");
    view.drag(view.query('[data-resize="right"]'), [1300, 300], [1200, 300]);
    await settle();
    assert.strictEqual(lastOf(arrangements(view))?.right.width, 400, "the inspector grows to the left");
  });

  test("the remembered arrangement is applied", async () => {
    const view = new Webview();
    await view.loaded();
    sized(view, 1600);
    view.send({
      type: "arrangement",
      arrangement: {
        left: { width: 300, collapsed: false },
        right: { width: 360, collapsed: true },
        bottom: { open: true, height: 260 },
      },
    });
    await settle();
    assert.ok(view.query('[data-part="inspector-strip"]'), "the inspector collapsed");
    const columns = view.query(".hb-body").style.gridTemplateColumns;
    assert.ok(columns.startsWith("300px"), columns);
    const tags = view.query(".hb-tags");
    assert.ok(tags, "the bottom panel open");
    assert.strictEqual(tags.style.height, "260px");
  });

  test("dragging the bottom panel's top edge resizes it", async () => {
    const view = new Webview();
    await view.loaded();
    sized(view, 1600);
    view.mouse("click", view.query('[data-action="toggle-tags"]'), 0, 0);
    await settle();
    const edge = view.query('[data-resize="bottom"]');
    assert.ok(edge, "a resize edge");
    view.drag(edge, [600, 700], [600, 620]);
    await settle();
    assert.strictEqual(lastOf(arrangements(view))?.bottom.height, 300);
  });

  test("a narrow editor shows the side panels as strips, and wide again the panels come back", async () => {
    const view = new Webview();
    await view.loaded();
    sized(view, 800);
    await settle();
    assert.ok(view.query('[data-part="components-strip"]'), "the Components strip");
    assert.ok(view.query('[data-part="inspector-strip"]'), "the inspector strip");
    sized(view, 1600);
    await settle();
    assert.ok(view.query('[data-component="gauge"]'), "the panels are back");
    assert.strictEqual(view.query('[data-part="components-strip"]'), null);
  });
});

suite("HMI Builder editor: Live mode", function () {
  this.timeout(10000);
  const rendered = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150));
  const lives = (view: Webview): any[] => view.posted.filter((message: any) => message.type === "live");
  const press = (view: Webview, action: string): void => {
    const button = view.query(`[data-action="${action}"]`);
    assert.ok(button, `the ${action} switch`);
    button.click();
  };

  test("Live mode asks for the page's bound paths and draws their values from the run target", async () => {
    const view = new Webview();
    await view.loaded();
    press(view, "live");
    await rendered();
    assert.deepStrictEqual(lastOf(lives(view)), { type: "live", on: true, paths: ["Main.P101.Speed"] });
    view.send({ type: "live", state: "connected", target: "Simulator" });
    view.send({
      type: "liveValues",
      update: { cycle: 1, values: { "Main.P101.Speed": { value: 1460 } }, missing: [] },
    });
    await rendered();
    assert.match(view.query('svg [data-object="speed"]')?.textContent ?? "", /1460/, "the live value");
    assert.match(view.query('[data-part="live-status"]')?.textContent ?? "", /Simulator/);
  });

  test("without a running target Live mode says so and shows every value as unknown", async () => {
    const view = new Webview();
    await view.loaded();
    press(view, "live");
    view.send({ type: "live", state: "none" });
    await rendered();
    assert.match(view.query('[data-part="live-status"]')?.textContent ?? "", /start the simulator/i);
    assert.strictEqual(view.query('svg [data-object="speed"]')?.getAttribute("data-state"), "unknown");
  });

  test("Design mode returns to the design samples and Live mode turns editing off", async () => {
    const view = new Webview();
    await view.loaded();
    press(view, "live");
    view.send({
      type: "liveValues",
      update: { cycle: 1, values: { "Main.P101.Speed": { value: 1460 } }, missing: [] },
    });
    await rendered();
    view.drag(view.query('svg [data-object="speed"]'), [100, 80], [180, 160]);
    await rendered();
    assert.strictEqual(view.edits().length, 0, "no edits in Live mode");
    press(view, "design");
    await rendered();
    assert.deepStrictEqual(lastOf(lives(view)), { type: "live", on: false });
    assert.doesNotMatch(view.query('svg [data-object="speed"]')?.textContent ?? "", /1460/, "the design sample again");
  });
});

suite("HMI Builder editor: Deploy", function () {
  this.timeout(10000);
  const rendered = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 150));
  const press = (view: Webview, action: string): void => {
    const button = view.query(`[data-action="${action}"]`);
    assert.ok(button, `the ${action} button`);
    button.click();
  };

  test("Deploy asks the host, then shows the version deployed and offers Roll back", async () => {
    const view = new Webview();
    await view.loaded();
    press(view, "deploy");
    await rendered();
    assert.ok(
      view.posted.some((message: any) => message.type === "deploy"),
      "the request",
    );
    view.send({ type: "deploying", action: "deploy" });
    await rendered();
    assert.strictEqual(view.query('[data-action="deploy"]')?.disabled, true, "one deployment at a time");
    view.send({ type: "deployed", action: "deploy", outcome: { ok: true, version: "abc123", target: "Line 1" } });
    await rendered();
    assert.match(view.query('[data-part="deploy-notice"]')?.textContent ?? "", /Deployed version abc123 to Line 1/);
    press(view, "rollback");
    await rendered();
    assert.ok(
      view.posted.some((message: any) => message.type === "rollback"),
      "the rollback",
    );
    view.send({ type: "deployed", action: "rollback", outcome: { ok: true, version: "old999", target: "Line 1" } });
    await rendered();
    const notice = view.query('[data-part="deploy-notice"]')?.textContent ?? "";
    assert.match(notice, /Rolled back to version old999 on Line 1/);
    assert.match(
      notice,
      /The HMI files of this project are unchanged/,
      "the rollback changes the runtime, not the files here",
    );
  });

  test("a refused deployment shows why, with each problem at its file and line", async () => {
    const view = new Webview();
    await view.loaded();
    view.send({
      type: "deployed",
      action: "deploy",
      outcome: {
        ok: false,
        reason: "The editor check found 1 problem.",
        problems: [
          { file: "hmi/pages/overview.toml", line: 12, message: '"Main.Gone" is not a variable of the program' },
        ],
      },
    });
    await rendered();
    const notice = view.query('[data-part="deploy-notice"]')?.textContent ?? "";
    assert.match(notice, /Not deployed: The editor check found 1 problem\./);
    assert.match(notice, /hmi\/pages\/overview\.toml:12/);
    assert.strictEqual(view.query('[data-action="rollback"]'), null, "nothing to roll back");
  });
});
