import * as assert from "assert";

import type { HmiObjectModel, HmiPageModel, PlacedFaceplate } from "../../hmiBuilder/drawing/model";
import { object, PAGE, Webview } from "./hmiBuilderWebview";

// Specification 39, §8.9: placing a faceplate on a page. The Components panel's Faceplates group
// lists the project's and its libraries' faceplates; one placed asks for its instance among those
// it fits and takes its own size; the canvas draws it bound as the display does; its inspector
// changes its instance; it resizes in proportion; a symbol's menu places its faceplate; the P&ID
// view offers none.

const FACEPLATES = [{ name: "pump-card", title: "Pump card", size: [320, 160] as [number, number] }];

function card(id: string, instance: string, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return object(id, "faceplate", [400, 300], [320, 204], { faceplate: "pump-card", instance, ...extra });
}

function bound(instance: string): PlacedFaceplate {
  return {
    model: {
      title: instance.split(".").pop() ?? instance,
      size: [320, 160],
      pipe: [],
      object: [
        object("running", "value", [16, 16], [160, 48], {
          label: "Running",
          bind: { value: { path: `${instance}.Running`, direction: "read" } },
        }),
      ],
    },
  };
}

async function withFaceplates(page: HmiPageModel = PAGE): Promise<Webview> {
  const view = new Webview();
  await view.loaded(page);
  view.send({ type: "faceplates", faceplates: FACEPLATES });
  return view;
}

const asked = (view: Webview, faceplate: string) =>
  view.posted.filter((message) => message.type === "faceplateInstances" && message.faceplate === faceplate);

suite("HMI Builder editor: placing faceplates", function () {
  this.timeout(20000);

  test("a placed PID inspector opens the configured Control Studio handoff without editing", async () => {
    const view = await withFaceplates({ ...PAGE, object: [card("pid", "Main.LIC101", { faceplate: "plant-pid" })] });
    view.drag(view.layer(), [450, 350], [450, 350]);
    const button = (await view.until("Open in Control Studio", () =>
      [...view.window.document.querySelectorAll("button")].find(
        (element: any) => element.textContent === "Open in Control Studio",
      ),
    )) as HTMLButtonElement;
    button.click();
    assert.ok(view.posted.some((message) => message.type === "openControlStudio" && message.id === "pid"));
    assert.strictEqual(view.edits().length, 0);
  });
  test("a faceplate from the panel asks for a fitting instance and is placed at its own size", async () => {
    const view = await withFaceplates();
    const entry = await view.until("the Faceplates group", () =>
      view.query('[data-component-group="faceplates"] [data-faceplate="pump-card"]'),
    );
    assert.ok(entry.textContent?.includes("Pump card"), entry.textContent);
    entry.click();
    await view.until("the instances asked for", () => asked(view, "pump-card").length > 0);
    view.send({ type: "faceplateInstances", faceplate: "pump-card", instances: ["Main.P1", "Main.P2"] });
    const chooser = await view.until(
      "the instance chooser with the answer",
      () =>
        view.query('[data-part="instance-chooser"]:has([data-instance])') ??
        (view.query('[data-part="instance-chooser"] [data-instance]') && view.query('[data-part="instance-chooser"]')),
    );
    assert.deepStrictEqual(
      [...chooser.querySelectorAll("[data-instance]")].map((button: any) => button.getAttribute("data-instance")),
      ["Main.P1", "Main.P2"],
    );
    chooser.querySelector('[data-instance="Main.P2"]').click();
    const edit = await view.nextEdit(0);
    const placed = edit.object.find((candidate) => candidate.type === "faceplate");
    assert.ok(placed, JSON.stringify(edit.object));
    assert.deepStrictEqual([placed.faceplate, placed.instance, placed.size], ["pump-card", "Main.P2", [320, 204]]);
    assert.ok(!view.query('[data-part="instance-chooser"]'), "the chooser closes");
  });

  test("when no instance fits, the chooser says so and nothing is placed", async () => {
    const view = await withFaceplates();
    (await view.until("the entry", () => view.query('[data-faceplate="pump-card"]'))).click();
    await view.until("asked", () => asked(view, "pump-card").length > 0);
    view.send({ type: "faceplateInstances", faceplate: "pump-card", instances: [] });
    const chooser = await view.until("the chooser's answer", () =>
      view.query('[data-part="instance-chooser"]')?.textContent?.includes("No instance of the program fits")
        ? view.query('[data-part="instance-chooser"]')
        : undefined,
    );
    chooser.querySelector('[data-action="cancel"]').click();
    await view.until("the chooser closed", () => !view.query('[data-part="instance-chooser"]'));
    assert.strictEqual(view.edits().length, 0);
  });

  test("the canvas draws a placed faceplate bound as the host gives it", async () => {
    const view = await withFaceplates({ ...PAGE, object: [...PAGE.object, card("card", "Main.P2")] });
    view.send({ type: "placed", placed: { card: bound("Main.P2") } });
    await view.until("the placed card", () => view.query('svg [data-placed="card"] [data-object="running"]'));
  });

  test("a placed faceplate's inspector names its faceplate and changes its instance among the fitting ones", async () => {
    const view = await withFaceplates({ ...PAGE, object: [card("card", "Main.P2")] });
    view.drag(view.layer(), [450, 350], [450, 350]);
    const section = await view.until("the faceplate section", () => view.query('[data-part="placed"]'));
    assert.ok(section.textContent?.includes("pump-card"), section.textContent);
    assert.strictEqual(view.query('[data-part="placed"] input[name="instance"]')?.value, "Main.P2");
    assert.ok(!view.query('input[name="label"]'), "a placement has no label of its own");
    assert.ok(!view.query('[data-part="preview"]'), "a placement has no states of its own to preview");
    view.query('[data-part="placed"] [data-action="pick"]').click();
    await view.until("asked", () => asked(view, "pump-card").length > 0);
    view.send({ type: "faceplateInstances", faceplate: "pump-card", instances: ["Main.P1", "Main.P2"] });
    (await view.until("the instance list", () => view.query('[data-part="placed"] [data-instance="Main.P1"]'))).click();
    const edit = await view.nextEdit(0);
    assert.strictEqual(edit.object[0].instance, "Main.P1");
  });

  test("resizing a placed faceplate keeps its proportions", async () => {
    const view = await withFaceplates({ ...PAGE, object: [card("card", "Main.P2")] });
    view.drag(view.layer(), [450, 350], [450, 350]);
    const handle = await view.until("the resize handles", () => view.query('[data-handle="se"]'));
    view.drag(handle, [720, 504], [880, 504]);
    const [width, height] = (await view.nextEdit(0)).object[0].size;
    assert.strictEqual(width, 480);
    assert.strictEqual(height, Math.round((480 * 204) / 320));
  });

  test("a symbol's menu places its faceplate on the page for its instance", async () => {
    const view = await withFaceplates({
      ...PAGE,
      object: [object("p1", "pump", [40, 40], [128, 108], { click: { faceplate: "pump-card", instance: "Main.P1" } })],
    });
    view.mouse("contextmenu", view.layer(), 80, 80, { button: 2 });
    (
      await view.until("place faceplate", () => view.query('[data-part="context-menu"] [data-menu="place-faceplate"]'))
    ).click();
    const edit = await view.nextEdit(0);
    const placed = edit.object.find((candidate) => candidate.type === "faceplate");
    assert.ok(placed, JSON.stringify(edit.object));
    assert.deepStrictEqual(
      [placed.faceplate, placed.instance, placed.size, placed.at],
      ["pump-card", "Main.P1", [320, 204], [192, 40]],
    );
  });

  test("Live mode reads the values a placed faceplate shows", async () => {
    const view = await withFaceplates({ ...PAGE, object: [card("card", "Main.P2")] });
    view.send({ type: "placed", placed: { card: bound("Main.P2") } });
    await view.until("the placed card", () => view.query('svg [data-placed="card"]'));
    (await view.until("the Live button", () => view.query('[data-action="live"]'))).click();
    const live = await view.until("Live mode on", () =>
      view.posted.find((message) => message.type === "live" && message.on === true),
    );
    assert.ok((live.paths as string[]).includes("Main.P2.Running"), JSON.stringify(live));
  });

  test("the P&ID view offers no faceplates", async () => {
    const view = new Webview();
    await view.ready();
    view.send({ type: "view", view: "pid" });
    await view.loaded(PAGE);
    view.send({ type: "faceplates", faceplates: FACEPLATES });
    await new Promise((resolve) => setTimeout(resolve, 50));
    assert.ok(!view.query('[data-component-group="faceplates"]'));
  });
});
